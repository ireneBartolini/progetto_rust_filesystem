async fn setup() {
    let client = reqwest::Client::new();

    // creation of some files and folders through API calls
    client.post("http://127.0.0.1:8080/mkdir/test_dir").send().await.unwrap();
    client.put("http://127.0.0.1:8080/files/test_dir/file1.txt").body("content").send().await.unwrap();
}

async fn cleanup() {
    let client = reqwest::Client::new();

    // delete all files and folders used for testing
    client.delete("http://127.0.0.1:8080/files/test_dir").send().await.unwrap();
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
    assert!(body.contains(&"file1.txt".to_string()));

    // cleanup
    cleanup().await
}

#[tokio::test]
async fn test_mkdir_api() {
    setup().await;

    let client = reqwest::Client::new();
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/new_dir")
        .send()
        .await
        .unwrap();

    // check the returned status
    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert!(body.contains("Directory created successfully"));

    // check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    
    // assert on the body of the response
    assert!(body.contains(&"new_dir".to_string()));




    cleanup().await;
}