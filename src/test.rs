use crate::{meta::Meta, Cache, Options};
use httpmock::Method::{GET, HEAD};
use httpmock::{mock, with_mock_server};
use reqwest::header::ETAG;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;

static ETAG_KEY: reqwest::header::HeaderName = ETAG;

#[test]
fn test_url_to_filename_with_etag() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource = "http://localhost:5000/foo.txt";
    let etag = String::from("abcd");

    assert_eq!(
        cache
            .resource_to_filepath(resource, &Some(etag), None, None)
            .to_str()
            .unwrap(),
        format!(
            "{}{}{}.{}",
            cache_dir.path().to_str().unwrap(),
            std::path::MAIN_SEPARATOR,
            "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
            "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
        )
    );
}

#[test]
fn test_url_to_filename_no_etag() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource = "http://localhost:5000/foo.txt";
    assert_eq!(
        cache
            .resource_to_filepath(resource, &None, None, None)
            .to_str()
            .unwrap(),
        format!(
            "{}{}{}",
            cache_dir.path().to_str().unwrap(),
            std::path::MAIN_SEPARATOR,
            "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
        )
    );
}

#[test]
fn test_url_to_filename_in_subdir() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource = "http://localhost:5000/foo.txt";
    assert_eq!(
        cache
            .resource_to_filepath(resource, &None, Some("target"), None)
            .to_str()
            .unwrap(),
        format!(
            "{}{}{}{}{}",
            cache_dir.path().to_str().unwrap(),
            std::path::MAIN_SEPARATOR,
            "target",
            std::path::MAIN_SEPARATOR,
            "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
        )
    );
}

#[test]
fn test_url_to_filename_with_suffix() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource = "http://localhost:5000/foo.txt";
    assert_eq!(
        cache
            .resource_to_filepath(resource, &None, Some("target"), Some("-extracted"))
            .to_str()
            .unwrap(),
        format!(
            "{}{}{}{}{}-extracted",
            cache_dir.path().to_str().unwrap(),
            std::path::MAIN_SEPARATOR,
            "target",
            std::path::MAIN_SEPARATOR,
            "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
        )
    );
}

#[test]
fn test_get_cached_path_local_file() {
    // Setup cache.
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let path = cache.cached_path("README.md").unwrap();
    assert_eq!(path, Path::new("README.md"));
}

#[test]
fn test_get_cached_path_non_existant_local_file_fails() {
    // Setup cache.
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let result = cache.cached_path("BLAH");
    assert!(result.is_err());
}

#[with_mock_server]
#[test]
fn test_cached_path() {
    // For debugging:
    // let _ = env_logger::try_init();

    // Setup cache.
    let cache_dir = tempdir().unwrap();
    let mut cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .freshness_lifetime(300)
        .build()
        .unwrap();

    let resource = "http://localhost:5000/resource.txt";

    // Mock the resource.
    let mut mock_1_head = mock(HEAD, "/resource.txt")
        .return_status(200)
        .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
        .create();
    let mut mock_1_get = mock(GET, "/resource.txt")
        .return_status(200)
        .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
        .return_body("Hello, World!")
        .create();

    // Get the cached path.
    let path = cache.cached_path(&resource[..]).unwrap();
    assert_eq!(
        path,
        cache.resource_to_filepath(&resource, &Some(String::from("fake-etag")), None, None)
    );

    assert_eq!(mock_1_head.times_called(), 1);
    assert_eq!(mock_1_get.times_called(), 1);

    // Ensure the file and meta exist.
    assert!(path.is_file());
    assert!(Meta::meta_path(&path).is_file());

    // Ensure the contents of the file are correct.
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(&contents[..], "Hello, World!");

    // When we attempt to get the resource again, the cache should still be fresh.
    let mut meta = Meta::from_cache(&path).unwrap();
    assert!(meta.is_fresh(None));
    let same_path = cache.cached_path(&resource[..]).unwrap();
    assert_eq!(same_path, path);
    assert!(path.is_file());
    assert!(Meta::meta_path(&path).is_file());

    // Didn't have to call HEAD or GET again.
    assert_eq!(mock_1_head.times_called(), 1);
    assert_eq!(mock_1_get.times_called(), 1);

    // Now expire the resource to continue testing.
    meta.expires = None;
    meta.to_file().unwrap();
    cache.freshness_lifetime = None;

    // After calling again when the resource is no longer fresh, the ETAG
    // should have been queried again with HEAD, but the resource should not have been
    // downloaded again with GET.
    let same_path = cache.cached_path(&resource[..]).unwrap();
    assert_eq!(same_path, path);
    assert!(path.is_file());
    assert!(Meta::meta_path(&path).is_file());
    assert_eq!(mock_1_head.times_called(), 2);
    assert_eq!(mock_1_get.times_called(), 1);

    // Now update the resource.
    mock_1_head.delete();
    mock_1_get.delete();
    let mock_2_head = mock(HEAD, "/resource.txt")
        .return_status(200)
        .return_header(&ETAG_KEY.to_string()[..], "fake-etag-2")
        .create();
    let mock_2_get = mock(GET, "/resource.txt")
        .return_status(200)
        .return_header(&ETAG_KEY.to_string()[..], "fake-etag-2")
        .return_body("Well hello again")
        .create();

    // Get the new cached path.
    let new_path = cache.cached_path(&resource[..]).unwrap();
    assert_eq!(
        new_path,
        cache.resource_to_filepath(&resource, &Some(String::from("fake-etag-2")), None, None)
    );

    assert_eq!(mock_2_head.times_called(), 1);
    assert_eq!(mock_2_get.times_called(), 1);

    // This should be different from the old path.
    assert_ne!(path, new_path);

    // Ensure the file and meta exist.
    assert!(new_path.is_file());
    assert!(Meta::meta_path(&new_path).is_file());

    // Ensure the contents of the file are correct.
    let new_contents = std::fs::read_to_string(&new_path).unwrap();
    assert_eq!(&new_contents[..], "Well hello again");
}

#[with_mock_server]
#[test]
fn test_cached_path_in_subdir() {
    // For debugging:
    // let _ = env_logger::try_init();

    // Setup cache.
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource = "http://localhost:5000/resource.txt";

    // Mock the resource.
    let mock_1_head = mock(HEAD, "/resource.txt")
        .return_status(200)
        .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
        .create();
    let mock_1_get = mock(GET, "/resource.txt")
        .return_status(200)
        .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
        .return_body("Hello, World!")
        .create();

    // Get the cached path.
    let path = cache
        .cached_path_with_options(&resource[..], &Options::default().subdir("target"))
        .unwrap();
    assert_eq!(
        path,
        cache.resource_to_filepath(
            &resource,
            &Some(String::from("fake-etag")),
            Some("target"),
            None
        )
    );

    assert_eq!(mock_1_head.times_called(), 1);
    assert_eq!(mock_1_get.times_called(), 1);

    // Ensure the file and meta exist.
    assert!(path.is_file());
    assert!(Meta::meta_path(&path).is_file());

    // Ensure the contents of the file are correct.
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(&contents[..], "Hello, World!");
}

#[test]
fn test_extract_tar_gz() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource: PathBuf = [
        ".",
        "test_fixtures",
        "utf-8_sample",
        "archives",
        "utf-8.tar.gz",
    ]
    .iter()
    .collect();

    let path = cache
        .cached_path_with_options(resource.to_str().unwrap(), &Options::default().extract())
        .unwrap();
    assert!(path.is_dir());
    assert!(path.to_str().unwrap().ends_with("-extracted"));
    assert!(path
        .to_str()
        .unwrap()
        .starts_with(cache_dir.path().to_str().unwrap()));
    let sample_file_path = path.join("dummy.txt");
    assert!(sample_file_path.is_file());
}

#[test]
fn test_extract_zip() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource: PathBuf = [
        ".",
        "test_fixtures",
        "utf-8_sample",
        "archives",
        "utf-8.zip",
    ]
    .iter()
    .collect();

    let path = cache
        .cached_path_with_options(resource.to_str().unwrap(), &Options::default().extract())
        .unwrap();
    assert!(path.is_dir());
    assert!(path.to_str().unwrap().ends_with("-extracted"));
    assert!(path
        .to_str()
        .unwrap()
        .starts_with(cache_dir.path().to_str().unwrap()));
    let sample_file_path = path.join("dummy.txt");
    assert!(sample_file_path.is_file());
}

#[test]
fn test_extract_in_subdir() {
    let cache_dir = tempdir().unwrap();
    let cache = Cache::builder()
        .dir(cache_dir.path().to_owned())
        .build()
        .unwrap();

    let resource: PathBuf = [
        ".",
        "test_fixtures",
        "utf-8_sample",
        "archives",
        "utf-8.tar.gz",
    ]
    .iter()
    .collect();

    let path = cache
        .cached_path_with_options(
            resource.to_str().unwrap(),
            &Options::default().subdir("target").extract(),
        )
        .unwrap();
    assert!(path.is_dir());
    assert!(path.to_str().unwrap().ends_with("-extracted"));
    assert!(path.parent().unwrap().to_str().unwrap().ends_with("target"));
    assert!(path
        .to_str()
        .unwrap()
        .starts_with(cache_dir.path().to_str().unwrap()));
    let sample_file_path = path.join("dummy.txt");
    assert!(sample_file_path.is_file());
}
