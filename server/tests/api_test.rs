async fn setup() {
    // creation of some files and folders
    let _ = std::fs::create_dir_all("remote-fs/test_dir");
    let _ = std::fs::write("remote-fs/test_dir/file.txt", "content");
}

async fn cleanup() {
    // delete all files and folders used for testing
    let _ = std::fs::remove_dir_all("remote-fs/test_dir");
}

#[tokio::test]
async fn test_list_dir_api() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    
    // assert on the body of the response
    assert!(body.contains(&"user".to_string()) || body.contains(&"file.txt".to_string()));

    // cleanup
    cleanup().await
}

#[tokio::test]
async fn test_mkdir_api() {
    let client = reqwest::Client::new();
    let res = client
        .post("http://127.0.0.1:8080/mkdir/home/nuova_dir")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert!(body.contains("Directory created successfully"));
}