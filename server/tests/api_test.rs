async fn setup() {
    let client = reqwest::Client::new();

    // creation of some files and folders through API calls
    client.post("http://127.0.0.1:8080/mkdir/test_dir").send().await.unwrap();
    client.put("http://127.0.0.1:8080/files/test_dir/file1.txt").body("content").send().await.unwrap();
    client.post("http://127.0.0.1:8080/mkdir/test_dir/dir1").send().await.unwrap();
}

async fn cleanup() {
    let client = reqwest::Client::new();

    // delete all files and folders used for testing
    client.delete("http://127.0.0.1:8080/files/test_dir").send().await.unwrap();
}

// TESTS ON
// GET /list/<path> – List directory contents

#[tokio::test]
#[serial_test::serial]
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
    assert!(body.contains(&"file1.txt".to_string()) && body.contains(&"dir1".to_string()));

    // cleanup
    cleanup().await
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_dot() {
    setup().await;

    let client = reqwest::Client::new();
    // List using /./ in the path
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/./dir1")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    // dir1 is empty from setup
    assert!(body.is_empty());

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_dotdot() {
    setup().await;

    let client = reqwest::Client::new();
    // List using /../ in the path (should resolve to test_dir)
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/dir1/../")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"file1.txt".to_string()) && body.contains(&"dir1".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_dot_and_dotdot_combo() {
    setup().await;

    let client = reqwest::Client::new();
    // List using a combination of /./ and /../
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/dir1/./.././")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"file1.txt".to_string()) && body.contains(&"dir1".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_list_dir_not_found_api() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/non_existant")
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup().await
}

// TESTS ON
// GET /files/<path> – Read file contents

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: String = res.text().await.unwrap();
    
    // assert on the body of the response
    assert!(body.contains(&"content".to_string()));

    // cleanup
    cleanup().await
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_dot() {
    setup().await;

    let client = reqwest::Client::new();
    // Read file using /./ in the path
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/./file1.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_dotdot() {
    setup().await;

    let client = reqwest::Client::new();
    // Read file using /../ in the path
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1/../file1.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_read_file_dot_and_dotdot_combo() {
    setup().await;

    let client = reqwest::Client::new();
    // Read file using a combination of /./ and /../
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1/.././file1.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_not_found() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_not_found.txt")
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup().await
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_wrong_path() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1/file1.txt")     // the file is bot in dir1
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup().await
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_directory_does_not_exist() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir2/file1.txt")     // the directory does not exist
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    // cleanup
    cleanup().await
}

#[tokio::test]
#[serial_test::serial]
async fn test_file_contents_bad_request() {
    // setup
    setup().await;

    // after the server is listening to 127.0.0.1:8080
    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/dir1")       // trying to read the content of a directory
        .send()
        .await
        .unwrap();

    // Check that the status is 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    // Check the body of the response
    let body = res.text().await.unwrap();
    assert!(body.contains("Invalid request"));

    // cleanup
    cleanup().await
}


// TESTS ON
// PUT /files/<path> – Write file contents

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_success() {
    setup().await;

    let client = reqwest::Client::new();
    // Write to a new file
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/new_file.txt")
        .body("hello world")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back the file to verify content
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/new_file.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "hello world");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_overwrite() {
    setup().await;

    let client = reqwest::Client::new();
    // Overwrite file1.txt
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .body("new content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back the file to verify new content
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "new content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_twice() {
    setup().await;

    let client = reqwest::Client::new();
    // First write
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .body("first content")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Second write
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .body("second content")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Read back the file to verify it contains the second content
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file1.txt")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "second content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_empty_content() {
    setup().await;

    let client = reqwest::Client::new();
    // Write empty content to a new file
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/empty.txt")
        .body("")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back the file to verify it is empty
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/empty.txt")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert_eq!(body, "");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_dot() {
    setup().await;

    let client = reqwest::Client::new();
    // Write using /./ in the path
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/./file_dot.txt")
        .body("dot content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back to verify
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_dot.txt")
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "dot content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_dotdot() {
    setup().await;

    let client = reqwest::Client::new();
    // Write using /../ in the path
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/dir1/../file_dotdot.txt")
        .body("dotdot content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back to verify
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_dotdot.txt")
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "dotdot content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_dot_and_dotdot_combo() {
    setup().await;

    let client = reqwest::Client::new();
    // Write using a combination of /./ and /../
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/dir1/.././file_combo.txt")
        .body("combo content")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Read back to verify
    let res = client
        .get("http://127.0.0.1:8080/files/test_dir/file_combo.txt")
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "combo content");

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_not_found() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to write to a file in a non-existent directory
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/non_existant_dir/file.txt")
        .body("should fail")
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_write_file_on_directory() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to write to a directory path
    let res = client
        .put("http://127.0.0.1:8080/files/test_dir/dir1")
        .body("should fail")
        .send()
        .await
        .unwrap();

    // Should return 400 bad request
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    let body = res.text().await.unwrap();
    assert!(body.contains("Invalid request"));

    cleanup().await;
}



// TESTS ON
// POST /mkdir/<path> – Create directory

#[tokio::test]
#[serial_test::serial]
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

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_dot() {
    setup().await;

    let client = reqwest::Client::new();
    // Create directory using /./ in the path
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/./new_dot_dir")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"new_dot_dir".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_dotdot() {
    setup().await;

    let client = reqwest::Client::new();
    // Create directory using /../ in the path
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/dir1/../new_dotdot_dir")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"new_dotdot_dir".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_dot_and_dotdot_combo() {
    setup().await;

    let client = reqwest::Client::new();
    // Create directory using a combination of /./ and /../
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/dir1/.././new_combo_dir")
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());

    // Check if the directory is actually there
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.contains(&"new_combo_dir".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_not_found() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory in a non-existent path
    let res = client
        .post("http://127.0.0.1:8080/mkdir/non_existant_dir/new_dir")
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_on_file() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory inside a file (should fail)
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/file1.txt/new_dir")
        .send()
        .await
        .unwrap();

    // Should return 400 bad request
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    let body = res.text().await.unwrap();
    assert!(body.contains("Invalid request"));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_mkdir_conflict() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to create a directory that already exists (should fail with conflict)
    let res = client
        .post("http://127.0.0.1:8080/mkdir/test_dir/dir1")
        .send()
        .await
        .unwrap();

    // Should return 409 conflict
    assert_eq!(res.status(), reqwest::StatusCode::CONFLICT);

    let body = res.text().await.unwrap();
    assert!(body.contains("already exists"));

    cleanup().await;
}


// TESTS ON
// DELETE /files/<path> – Delete file or directory

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_success() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/delete_me.txt")
        .body("to be deleted")
        .send()
        .await
        .unwrap();

    // Delete the file
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/delete_me.txt")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"delete_me.txt".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_directory_success() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a directory
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/delete_dir")
        .send()
        .await
        .unwrap();

    // Delete the directory
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/delete_dir")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the directory is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"delete_dir".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_nested_directory_success() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a nested directory structure
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer")
        .send()
        .await
        .unwrap();
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer/inner")
        .send()
        .await
        .unwrap();
    client
        .put("http://127.0.0.1:8080/files/test_dir/outer/inner/file.txt")
        .body("nested content")
        .send()
        .await
        .unwrap();

    // Delete the outer directory
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/outer")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the outer directory is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"outer".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_and_recreate_directory() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a nested directory structure
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer")
        .send()
        .await
        .unwrap();
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer/inner")
        .send()
        .await
        .unwrap();
    client
        .put("http://127.0.0.1:8080/files/test_dir/outer/inner/file.txt")
        .body("nested content")
        .send()
        .await
        .unwrap();

    // Delete the outer directory
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/outer")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Recreate the outer directory
    client
        .post("http://127.0.0.1:8080/mkdir/test_dir/outer")
        .send()
        .await
        .unwrap();

    // List contents of the recreated directory (should be empty)
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir/outer")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(body.is_empty());

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_dot() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/file_dot_delete.txt")
        .body("delete me")
        .send()
        .await
        .unwrap();

    // Delete using /./ in the path
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/./file_dot_delete.txt")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"file_dot_delete.txt".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_dotdot() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/file_dotdot_delete.txt")
        .body("delete me")
        .send()
        .await
        .unwrap();

    // Delete using /../ in the path
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/dir1/../file_dotdot_delete.txt")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"file_dotdot_delete.txt".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_dot_and_dotdot_combo() {
    setup().await;

    let client = reqwest::Client::new();
    // Create a file
    client
        .put("http://127.0.0.1:8080/files/test_dir/file_combo_delete.txt")
        .body("delete me")
        .send()
        .await
        .unwrap();

    // Delete using a combination of /./ and /../
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/dir1/.././file_combo_delete.txt")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Check with list that the file is gone
    let res = client
        .get("http://127.0.0.1:8080/list/test_dir")
        .send()
        .await
        .unwrap();
    let body: Vec<String> = res.json().await.unwrap();
    assert!(!body.contains(&"file_combo_delete.txt".to_string()));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_file_not_found() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to delete a file that does not exist
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/does_not_exist.txt")
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_delete_directory_not_found() {
    setup().await;

    let client = reqwest::Client::new();
    // Try to delete a directory that does not exist
    let res = client
        .delete("http://127.0.0.1:8080/files/test_dir/does_not_exist_dir")
        .send()
        .await
        .unwrap();

    // Should return 404 not found
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

    let body = res.text().await.unwrap();
    assert!(body.contains("not found"));

    cleanup().await;
}