


async fn setup()-> String{
    let client = reqwest::Client::new();

    client.post("http://127.0.0.1:8080/auth/register")
    .json(&serde_json::json!({
        "username": "testuser",
        "password": "password"
    }))
    .send()
    .await
    .unwrap();

    let res = client.post("http://127.0.0.1:8080/auth/login")
    .json(&serde_json::json!({
        "username": "testuser",
        "password": "password"
    }))
    .send()
    .await
    .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["token"].as_str().unwrap();



    // creation of some files and folders through API calls
    client
    .post("http://127.0.0.1:8080/mkdir/test_dir")
    .bearer_auth(&token)
    .send()
    .await
    .unwrap();
    
    client
    .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
    .bearer_auth(&token)
    .body("content")
    .send()
    .await
    .unwrap();

    client.post("http://127.0.0.1:8080/mkdir/test_dir/dir1")
    .bearer_auth(&token)
    .send()
    .await.
    unwrap();

    token.to_string()

}

async fn cleanup(token: String) {
    let client = reqwest::Client::new();

    // delete all files and folders used for testing
    client.delete("http://127.0.0.1:8080/files/test_dir")
    .bearer_auth(token)
    .send().await.unwrap();
}

// TESTS ON
// GET /list/<path> – List directory contents

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_api() {
    // setup
    let token= setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    
    // assert on the body of the response
    assert!(body.contains(&"file1.txt".to_string()) && body.contains(&"dir1".to_string()));

    // cleanup
    cleanup(token).await
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_dot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // List using /./ in the path
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/./dir1")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    // dir1 is empty from setup
    assert!(body.is_empty());

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_dotdot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // List using /../ in the path (should resolve to test_dir)
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/dir1/../")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"file1.txt".to_string()) && body.contains(&"dir1".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_dot_and_dotdot_combo() {
    let token =setup().await;

    let client = reqwest::Client::new();
    // List using a combination of /./ and /../
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/dir1/./.././")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"file1.txt".to_string()) && body.contains(&"dir1".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_not_found_api() {
    // setup
    let token=setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/non_existant")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup(token).await
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_outside_root() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to list a directory outside the root
    let res = client
        .get("http://127.0.0.1:8080/list/../../etc")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    cleanup(token).await;
}

// TESTS ON
// GET /files/<path> – Read file contents

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents() {
    // setup
    let token=setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: String = res.text().await.unwrap();
    
    // assert on the body of the response
    assert!(body.contains(&"content".to_string()));

    // cleanup
    cleanup(token).await
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_dot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Read file using /./ in the path
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/./file1.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_dotdot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Read file using /../ in the path
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1/../file1.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_dot_and_dotdot_combo() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Read file using a combination of /./ and /../
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1/.././file1.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_not_found() {
    // setup
    let token=setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_not_found.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup(token).await
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_wrong_path() {
    // setup
    let token=setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1/file1.txt")     // the file is bot in dir1
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup(token).await
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_directory_does_not_exist() {
    // setup
    let token=setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir2/file1.txt")     // the directory does not exist
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup(token).await
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_outside_root() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to read a file outside the root
    let res = client
        .get("http://127.0.0.1:8080/files/../../etc/passwd")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_bad_request() {
    // setup
    let token=setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1")       // trying to read the content of a directory
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("Invalid request"));

    // cleanup
    cleanup(token).await
}


// TESTS ON
// PUT /files/<path> – Write file contents

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_success() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Write to a new file
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/new_file.txt")
        .bearer_auth(&token)
        .body("hello world")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back the file to verify content
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/new_file.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "hello world");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_overwrite() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Overwrite file1.txt
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .bearer_auth(&token)
        .body("new content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back the file to verify new content
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "new content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_twice() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // First write
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .bearer_auth(&token)
        .body("first content")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Second write
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .bearer_auth(&token)
        .body("second content")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Read back the file to verify it contains the second content
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "second content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_empty_content() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Write empty content to a new file
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/empty.txt")
        .bearer_auth(&token)
        .body("")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back the file to verify it is empty
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/empty.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_dot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Write using /./ in the path
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/./file_dot.txt")
        .bearer_auth(&token)
        .body("dot content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back to verify
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_dot.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "dot content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_dotdot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Write using /../ in the path
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/dir1/../file_dotdot.txt")
        .bearer_auth(&token)
        .body("dotdot content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back to verify
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_dotdot.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "dotdot content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_dot_and_dotdot_combo() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Write using a combination of /./ and /../
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/dir1/.././file_combo.txt")
        .bearer_auth(&token)
        .body("combo content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back to verify
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_combo.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "combo content");

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_not_found() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to write to a file in a non-existent directory
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/non_existant_dir/file.txt")
        .bearer_auth(&token)
        .body("should fail")
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_outside_root() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to write a file outside the root
    let res = client
        .put("http://127.0.0.1:8080/files/../../outside.txt")
        .bearer_auth(&token)
        .body("should not be created")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_on_directory() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to write to a directory path
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/dir1")
        .bearer_auth(&token)
        .body("should fail")
        .send()
        .await
        .unwrap();

    // Should return 400 bad request
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    let body = res.text().await.unwrap();
    assert!(body.contains("Invalid request"));

    cleanup(token).await;
}



// TESTS ON
// POST /mkdir/<path> – Create directory

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_api() {
    let token=setup().await;

    let client = reqwest::Client::new();
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/new_dir")
        .bearer_auth(&token)
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
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    
    // assert on the body of the response
    assert!(body.contains(&"new_dir".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_dot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create directory using /./ in the path
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/./new_dot_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"new_dot_dir".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_dotdot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create directory using /../ in the path
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/dir1/../new_dotdot_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"new_dotdot_dir".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_dot_and_dotdot_combo() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create directory using a combination of /./ and /../
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/dir1/.././new_combo_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"new_combo_dir".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_not_found() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory in a non-existent path
    let res = client
        .post("http://127.0.0.1:8080/mkdir/non_existant_dir/new_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_outside_root() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory outside the root
    let res = client
        .post("http://127.0.0.1:8080/mkdir/../../outside_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_on_file() {
   let token= setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory inside a file (should fail)
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/file1.txt/new_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Should return 400 bad request
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    let body = res.text().await.unwrap();
    assert!(body.contains("Invalid request"));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_conflict() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory that already exists (should fail with conflict)
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/dir1")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Should return 409 conflict
    assert_eq!(res.status(), reqwest::StatusCode::CONFLICT);

    let body = res.text().await.unwrap();
    assert!(body.contains("already exists"));

    cleanup(token).await;
}


// TESTS ON
// DELETE /files/<path> – Delete file or directory

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_success() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/delete_me.txt")
        .bearer_auth(&token)
        .body("to be deleted")
        .send()
        .await
        .unwrap();

    // Delete the file
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/delete_me.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"delete_me.txt".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_directory_success() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create a directory
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/delete_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Delete the directory
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/delete_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the directory is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"delete_dir".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_nested_directory_success() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create a nested directory structure
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer/inner")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    client
        .put("http://127.0.0.1:8080/files/test_dir/outer/inner/file.txt")
        .bearer_auth(&token)
        .body("nested content")
        .send()
        .await
        .unwrap();

    // Delete the outer directory
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/outer")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the outer directory is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"outer".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_and_recreate_directory() {
   let token= setup().await;

    let client = reqwest::Client::new();
    // Create a nested directory structure
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer/inner")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    client
        .put("http://127.0.0.1:8080/files/test_dir/outer/inner/file.txt")
        .bearer_auth(&token)
        .body("nested content")
        .send()
        .await
        .unwrap();

    // Delete the outer directory
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/outer")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Recreate the outer directory
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // List contents of the recreated directory (should be empty)
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/outer")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.is_empty());

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_dot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/file_dot_delete.txt")
        .bearer_auth(&token)
        .body("delete me")
        .send()
        .await
        .unwrap();

    // Delete using /./ in the path
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/./file_dot_delete.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"file_dot_delete.txt".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_dotdot() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/file_dotdot_delete.txt")
        .bearer_auth(&token)
        .body("delete me")
        .send()
        .await
        .unwrap();

    // Delete using /../ in the path
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/dir1/../file_dotdot_delete.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"file_dotdot_delete.txt".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_dot_and_dotdot_combo() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/file_combo_delete.txt")
        .bearer_auth(&token)
        .body("delete me")
        .send()
        .await
        .unwrap();

    // Delete using a combination of /./ and /../
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/dir1/.././file_combo_delete.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"file_combo_delete.txt".to_string()));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_not_found() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to delete a file that does not exist
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/does_not_exist.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_outside_root() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to delete a file outside the root
    let res = client
        .delete("http://127.0.0.1:8080/files/../../outside.txt")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    cleanup(token).await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_directory_not_found() {
    let token=setup().await;

    let client = reqwest::Client::new();
    // Try to delete a directory that does not exist
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/does_not_exist_dir")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup(token).await;
}