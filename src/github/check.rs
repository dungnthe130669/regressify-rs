pub fn is_github() -> bool {
    std::env::var("GITHUB_ACTIONS").unwrap_or_default() == "true"
        && std::env::var("GITHUB_WORKSPACE").is_ok()
        && std::env::var("GITHUB_REPOSITORY").is_ok()
        && std::env::var("GITHUB_RUN_ID").is_ok()
        && std::env::var("GITHUB_RUN_NUMBER").is_ok()
}
