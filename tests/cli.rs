use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command; // Run programs

#[test]
fn file_doesnt_exist() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("cached-path")?;

    cmd.arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("file does not exist"));

    Ok(())
}

#[test]
fn test_remote_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("cached-path")?;

    cmd.arg("https://raw.githubusercontent.com/epwalsh/rust-cached-path/master/test_fixtures/utf-8_sample/utf-8_sample.txt");
    let result = cmd.assert().success();
    let output = result.get_output();
    let mut stdout = String::from_utf8(output.stdout.clone()).unwrap();
    // remove newline at the end.
    stdout.pop();
    let path = PathBuf::from(stdout);
    println!("{:?}", path);
    assert!(path.is_file());

    // Ensure cached version exactly matches local version.
    let cached_file = fs::File::open(&path)?;
    let cached_bytes: Result<Vec<u8>, _> = cached_file.bytes().collect();
    let cached_bytes = cached_bytes.unwrap();

    let local_path: PathBuf = [".", "test_fixtures", "utf-8_sample", "utf-8_sample.txt"]
        .iter()
        .collect();
    assert!(local_path.is_file());
    let local_file = fs::File::open(local_path)?;
    let local_bytes: Result<Vec<u8>, _> = local_file.bytes().collect();
    let local_bytes = local_bytes.unwrap();

    assert!(cached_bytes == local_bytes);

    Ok(())
}
