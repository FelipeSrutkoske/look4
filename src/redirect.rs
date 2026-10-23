use std::collections::HashSet;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub struct RedirectHop {
    pub url: String,
    pub status: u16,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct TraceResult {
    pub initial_url: String,
    pub final_url: String,
    pub hops: Vec<RedirectHop>,
    pub total_redirects: usize,
    pub is_same_domain: bool,
    pub final_status: u16,
    pub final_reason: String,
}

pub async fn run_in_background<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (sender, receiver) = futures_channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = sender.send(operation());
    });
    receiver
        .await
        .map_err(|_| "A consulta foi interrompida antes de terminar.".to_string())?
}

pub fn trace(raw_url: &str) -> Result<TraceResult, String> {
    let mut current_url = Url::parse(raw_url.trim()).map_err(|e| format!("URL inválida: {e}"))?;

    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Look4/0.1.0",
        )
        .build()
        .map_err(|e| format!("Erro ao inicializar conexão: {e}"))?;

    let initial_url = current_url.to_string();
    let mut hops = Vec::new();
    let mut visited = HashSet::new();
    const MAX_HOPS: usize = 20;

    let mut final_status = 200;
    let mut final_reason = "OK".to_string();

    for _ in 0..MAX_HOPS {
        let current_str = current_url.to_string();
        if visited.contains(&current_str) {
            hops.push(RedirectHop {
                url: current_str.clone(),
                status: 310,
                reason: "Loop de redirecionamento detectado".to_string(),
            });
            break;
        }
        visited.insert(current_str.clone());

        let response = match client.get(current_url.clone()).send() {
            Ok(resp) => resp,
            Err(err) => {
                if hops.is_empty() {
                    return Err(format!("Falha ao conectar: {err}"));
                } else {
                    hops.push(RedirectHop {
                        url: current_str.clone(),
                        status: 0,
                        reason: format!("Erro de conexão no salto: {err}"),
                    });
                    break;
                }
            }
        };

        let status = response.status();
        let status_code = status.as_u16();
        let reason = status
            .canonical_reason()
            .unwrap_or("Desconhecido")
            .to_string();

        if status.is_redirection() || response.headers().contains_key(reqwest::header::LOCATION) {
            let next_url = if let Some(loc) = response.headers().get(reqwest::header::LOCATION) {
                let loc_str = loc
                    .to_str()
                    .map_err(|_| "Cabeçalho de redirecionamento inválido".to_string())?;
                current_url
                    .join(loc_str)
                    .map_err(|e| format!("URL de redirecionamento inválida ({loc_str}): {e}"))?
            } else {
                hops.push(RedirectHop {
                    url: current_str.clone(),
                    status: status_code,
                    reason: "Redirecionamento sem endereço de destino".to_string(),
                });
                break;
            };

            hops.push(RedirectHop {
                url: current_str.clone(),
                status: status_code,
                reason,
            });
            current_url = next_url;
            continue;
        }

        // Check for HTML meta refresh if 200 OK and text/html
        let is_html = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .map(|ct| ct.contains("text/html"))
            .unwrap_or(false);

        if status.is_success() && is_html {
            use std::io::Read;
            let mut chunk = vec![0u8; 4096];
            let mut reader = response;
            let bytes_read = reader.read(&mut chunk).unwrap_or(0);
            let body_prefix = String::from_utf8_lossy(&chunk[..bytes_read]);
            if let Some(target) = extract_meta_refresh(&body_prefix) {
                if let Ok(next_url) = current_url.join(&target) {
                    if next_url != current_url {
                        hops.push(RedirectHop {
                            url: current_str.clone(),
                            status: status_code,
                            reason: "Meta Refresh (HTML)".to_string(),
                        });
                        current_url = next_url;
                        continue;
                    }
                }
            }
        }

        final_status = status_code;
        final_reason = reason.clone();
        hops.push(RedirectHop {
            url: current_str.clone(),
            status: status_code,
            reason,
        });
        break;
    }

    let final_url = current_url.to_string();
    let total_redirects = hops.len().saturating_sub(1);
    let is_same_domain = match (Url::parse(&initial_url), Url::parse(&final_url)) {
        (Ok(u1), Ok(u2)) => u1.domain() == u2.domain() && u1.domain().is_some(),
        _ => false,
    };

    Ok(TraceResult {
        initial_url,
        final_url,
        hops,
        total_redirects,
        is_same_domain,
        final_status,
        final_reason,
    })
}

fn extract_meta_refresh(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let refresh_idx = lower
        .find("http-equiv=\"refresh\"")
        .or_else(|| lower.find("http-equiv='refresh'"))
        .or_else(|| lower.find("http-equiv=refresh"))?;

    let tag_start = lower[..refresh_idx].rfind('<')?;
    let tag_end = lower[refresh_idx..].find('>')? + refresh_idx;
    let tag = &html[tag_start..=tag_end];
    let tag_lower = tag.to_lowercase();

    let url_marker = tag_lower.find("url=")?;
    let after_url = &tag[url_marker + 4..];
    let trimmed = after_url.trim_start_matches([' ', '\'', '"']);
    let url_end = trimmed
        .find(['\'', '"', ';', ' ', '>'])
        .unwrap_or(trimmed.len());
    let target = &trimmed[..url_end];
    if target.is_empty() {
        None
    } else {
        Some(target.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_meta_refresh() {
        let html1 = r#"<html><head><meta http-equiv="refresh" content="0; url=https://example.com/target"></head></html>"#;
        assert_eq!(
            extract_meta_refresh(html1),
            Some("https://example.com/target".to_string())
        );

        let html2 = r#"<html><head><meta http-equiv='refresh' content='2;url=/relative/path'></head></html>"#;
        assert_eq!(
            extract_meta_refresh(html2),
            Some("/relative/path".to_string())
        );

        let html3 = r#"<html><head><title>No refresh</title></head></html>"#;
        assert_eq!(extract_meta_refresh(html3), None);

        let html4 = r#"<meta http-equiv="refresh" content="5; URL='http://test.org/final'">"#;
        assert_eq!(
            extract_meta_refresh(html4),
            Some("http://test.org/final".to_string())
        );
    }

    #[test]
    fn test_trace_invalid_url() {
        assert!(trace("not a url").is_err());
        assert!(trace("").is_err());
    }
}
