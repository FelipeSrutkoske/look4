use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use std::time::Duration;

const API_BASE: &str = "https://www.virustotal.com/api/v3";

pub enum Lookup {
    Report { report: Report, report_url: String },
    NeedsSubmission { report_url: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Safe,
    Suspicious,
    Malicious,
}

pub struct Report {
    pub verdict: Verdict,
    pub malicious: u32,
    pub suspicious: u32,
    pub harmless: u32,
    pub undetected: u32,
    pub timeout: u32,
    pub date: Option<String>,
}

impl Report {
    pub fn total(&self) -> u32 {
        self.malicious + self.suspicious + self.harmless + self.undetected + self.timeout
    }
}

#[derive(Deserialize)]
struct Envelope {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    id: Option<String>,
    attributes: Option<Attributes>,
}

#[derive(Deserialize)]
struct Attributes {
    stats: Option<Stats>,
    last_analysis_stats: Option<Stats>,
    last_analysis_date: Option<i64>,
    status: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Stats {
    malicious: u32,
    suspicious: u32,
    harmless: u32,
    undetected: u32,
    timeout: u32,
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

pub fn lookup(url: &str, api_key: &str) -> Result<Lookup, String> {
    let client = build_client()?;
    let url_id = URL_SAFE_NO_PAD.encode(url.as_bytes());
    let report_url = format!("https://www.virustotal.com/gui/url/{url_id}");
    let response = client
        .get(format!("{API_BASE}/urls/{url_id}"))
        .header("x-apikey", api_key)
        .send()
        .map_err(|error| network_error("consultar o VirusTotal", error))?;

    if response.status().is_success() {
        let report: Envelope = response
            .json()
            .map_err(|error| format!("Resposta inválida do VirusTotal: {error}"))?;
        return Ok(Lookup::Report {
            report: build_report(report),
            report_url,
        });
    }
    if response.status().as_u16() == 404 {
        return Ok(Lookup::NeedsSubmission { report_url });
    }
    Err(api_error(response))
}

pub fn submit_and_poll(
    url: &str,
    api_key: &str,
    report_url: &str,
) -> Result<(Report, String), String> {
    let client = build_client()?;
    let response = client
        .post(format!("{API_BASE}/urls"))
        .header("x-apikey", api_key)
        .form(&[("url", url)])
        .send()
        .map_err(|error| network_error("enviar a URL", error))?;
    if !response.status().is_success() {
        return Err(api_error(response));
    }
    let submission: Envelope = response
        .json()
        .map_err(|error| format!("Resposta inválida ao enviar URL: {error}"))?;
    let analysis_id = submission
        .data
        .id
        .ok_or("O VirusTotal não retornou o identificador da análise.")?;

    for _ in 0..12 {
        std::thread::sleep(Duration::from_secs(5));
        let response = client
            .get(format!(
                "{API_BASE}/analyses/{}",
                encode_path_segment(&analysis_id)
            ))
            .header("x-apikey", api_key)
            .send()
            .map_err(|error| network_error("acompanhar a análise", error))?;
        if !response.status().is_success() {
            return Err(api_error(response));
        }
        let analysis: Envelope = response
            .json()
            .map_err(|error| format!("Resposta inválida da análise: {error}"))?;
        if let Some(attributes) = analysis.data.attributes {
            if attributes.status.as_deref() == Some("completed") {
                let stats = attributes.stats.unwrap_or_default();
                return Ok((report_from_stats(&stats, None), report_url.to_string()));
            }
        }
    }
    Err("A análise continua pendente. Tente consultar novamente daqui a pouco.".into())
}

fn build_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Não foi possível preparar a conexão: {error}"))
}

fn network_error(action: &str, error: reqwest::Error) -> String {
    if error.is_timeout() {
        format!("A conexão expirou ao {action}. Verifique a rede e tente novamente.")
    } else {
        format!("Falha de rede ao {action}: {error}")
    }
}

fn build_report(report: Envelope) -> Report {
    let attributes = report.data.attributes.unwrap_or(Attributes {
        stats: None,
        last_analysis_stats: None,
        last_analysis_date: None,
        status: None,
    });
    let stats = attributes
        .last_analysis_stats
        .or(attributes.stats)
        .unwrap_or_default();
    let date = attributes
        .last_analysis_date
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|date| format!("{} UTC", date.format("%d/%m/%Y %H:%M")));
    report_from_stats(&stats, date)
}

fn report_from_stats(stats: &Stats, date: Option<String>) -> Report {
    let verdict = if stats.malicious > 0 {
        Verdict::Malicious
    } else if stats.suspicious > 0 {
        Verdict::Suspicious
    } else {
        Verdict::Safe
    };
    Report {
        verdict,
        malicious: stats.malicious,
        suspicious: stats.suspicious,
        harmless: stats.harmless,
        undetected: stats.undetected,
        timeout: stats.timeout,
        date,
    }
}

fn api_error(response: reqwest::blocking::Response) -> String {
    match response.status().as_u16() {
        401 | 403 => "Chave da API inválida ou sem permissão para esta consulta.".into(),
        429 => "Limite da API do VirusTotal atingido. Aguarde e tente mais tarde.".into(),
        400 => "O VirusTotal recusou a URL enviada.".into(),
        code => format!("Erro do VirusTotal (HTTP {code})."),
    }
}

fn encode_path_segment(value: &str) -> String {
    value
        .replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}
