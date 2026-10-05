const APP_ID: &str = "br.com.look4.LinkChecker";
const API_KEY_ACCOUNT: &str = "virustotal-api-key";

pub fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(APP_ID, API_KEY_ACCOUNT).map_err(|error| error.to_string())
}
