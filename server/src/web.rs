//! Serves the built web app from `REVIT_DOWNGRADE_WEB_DIR`.
//!
//! The build prerenders every page to `pages/<path>.html` (`pages/index.html` for `/`) and a `pages/404.html`. A page's
//! address returns its own HTML; a job's address (`/job/<id>`) returns the app shell, kept out of search; any other
//! address that is not a file returns the 404 page with status 404. Addresses with a trailing slash and `/index.html`
//! redirect to the one address of their page. A web directory without `pages/` (an older build) falls back to serving
//! `index.html` for every unknown address, as a plain single-page app.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use std::path::{Path, PathBuf};
use tower_http::services::ServeDir;

/// Hashed build output: its name changes whenever its content does.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Pages and the app shell: always checked with the server, so a release shows at once.
const REVALIDATE: &str = "no-cache";
/// Pictures (icons, the link-preview image): they rarely change, so they are cached for a week.
const IMAGE: &str = "public, max-age=604800";
/// Other files (robots.txt, sitemap.xml, llms.txt): cached for an hour.
const SHORT: &str = "public, max-age=3600";

fn is_image(path: &str) -> bool {
    [".png", ".ico", ".svg", ".jpg", ".webp"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

pub async fn serve(dir: PathBuf, req: Request) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();

    // One address per page.
    if path == "/index.html" {
        return Redirect::permanent(&format!("/{query}")).into_response();
    }
    if path.len() > 1 && path.ends_with('/') {
        let trimmed = path.trim_end_matches('/');
        let target = if trimmed.is_empty() { "/" } else { trimmed };
        return Redirect::permanent(&format!("{target}{query}")).into_response();
    }

    let pages = dir.join("pages");
    let prerendered = tokio::fs::metadata(&pages).await.is_ok_and(|m| m.is_dir());
    // The prerendered files are reached through their page's address only.
    if path == "/pages" || path.starts_with("/pages/") {
        return not_found(&dir, &path, prerendered).await;
    }
    if prerendered
        && let Some(file) = page_file(&pages, &path)
        && let Some(resp) = html_file(&file, StatusCode::OK).await
    {
        return resp;
    }
    if is_job_path(&path)
        && let Some(mut resp) = html_file(&dir.join("index.html"), StatusCode::OK).await
    {
        resp.headers_mut()
            .insert("x-robots-tag", HeaderValue::from_static("noindex"));
        return resp;
    }

    // A real file of the build: an asset, robots.txt, sitemap.xml, an icon.
    if path != "/" {
        let cache = if path.starts_with("/assets/") {
            IMMUTABLE
        } else if is_image(&path) {
            IMAGE
        } else {
            SHORT
        };
        match ServeDir::new(&dir)
            .append_index_html_on_directories(false)
            .try_call(req)
            .await
        {
            Ok(resp) if resp.status() != StatusCode::NOT_FOUND => {
                let mut resp = resp.map(Body::new);
                if resp.status().is_success() || resp.status() == StatusCode::NOT_MODIFIED {
                    resp.headers_mut()
                        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
                }
                return resp;
            }
            Ok(_) => {}
            // `try_call` reports a missing file as an error; an address that is no file is simply not found.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(error = %e, "reading a web file failed");
            }
        }
    }
    not_found(&dir, &path, prerendered).await
}

/// `pages/<path>.html` for a page address made only of lowercase letters, digits and dashes; `pages/index.html` for `/`.
fn page_file(pages: &Path, path: &str) -> Option<PathBuf> {
    if path == "/" {
        return Some(pages.join("index.html"));
    }
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let valid = segments.iter().all(|s| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    });
    if !valid || segments.contains(&"404") {
        return None;
    }
    let mut file = pages.to_path_buf();
    for (i, s) in segments.iter().enumerate() {
        if i + 1 == segments.len() {
            file.push(format!("{s}.html"));
        } else {
            file.push(s);
        }
    }
    Some(file)
}

/// `/job/<id>`: a conversion's own address, opened from the visitor's history or a link they keep.
fn is_job_path(path: &str) -> bool {
    path.strip_prefix("/job/")
        .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

async fn html_file(file: &Path, status: StatusCode) -> Option<Response> {
    let body = tokio::fs::read(file).await.ok()?;
    let mut resp = (status, body).into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE));
    Some(resp)
}

async fn not_found(dir: &Path, path: &str, prerendered: bool) -> Response {
    if prerendered {
        // A missing Vietnamese page (/vi/...) gets the 404 page in Vietnamese.
        if (path == "/vi" || path.starts_with("/vi/"))
            && let Some(resp) = html_file(
                &dir.join("pages").join("vi").join("404.html"),
                StatusCode::NOT_FOUND,
            )
            .await
        {
            return resp;
        }
        if let Some(resp) =
            html_file(&dir.join("pages").join("404.html"), StatusCode::NOT_FOUND).await
        {
            return resp;
        }
        return StatusCode::NOT_FOUND.into_response();
    }
    // An older build without prerendered pages: the app routes every address itself.
    html_file(&dir.join("index.html"), StatusCode::OK)
        .await
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}
