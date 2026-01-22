pub fn is_azure_devops() -> bool {
    std::env::var("TF_BUILD").is_ok()
        && std::env::var("SYSTEM_TEAMFOUNDATIONSERVERURI").is_ok()
        && std::env::var("SYSTEM_TEAMFOUNDATIONCOLLECTIONURI").is_ok()
        && std::env::var("SYSTEM_TEAMPROJECT").is_ok()
        && std::env::var("SYSTEM_COLLECTIONURI").is_ok()
}
