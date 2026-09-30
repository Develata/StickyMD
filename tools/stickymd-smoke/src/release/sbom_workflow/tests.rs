use super::*;

#[test]
fn download_failures_retry_with_the_same_pin_and_preserve_old_cache() {
    let temp = TemporaryDirectory::new("download-test").unwrap();
    let (plan, _) = syft::prepare(temp.path(), None, &temp.path().join("verified")).unwrap();
    let download = &plan.downloads[0];
    fs::create_dir_all(download.path.parent().unwrap()).unwrap();
    fs::write(&download.path, "previous cache").unwrap();
    for transport_failure in [false, true] {
        let mut paths = Vec::new();
        let mut waits = Vec::new();
        let error = download_with(
            download,
            temp.path(),
            |path| {
                paths.push(path.to_path_buf());
                fs::write(path, "incomplete or corrupt bytes").unwrap();
                if transport_failure {
                    Err("transport interrupted".into())
                } else {
                    syft::publish_cached(temp.path(), download.kind, path)
                }
            },
            |seconds| waits.push(seconds),
        )
        .unwrap_err();
        assert!(
            error.contains("download failed after 3 attempts"),
            "{error}"
        );
        assert_eq!(paths.len(), 3);
        assert_eq!(waits, [1, 2]);
        assert!(paths.windows(2).all(|p| p[0] != p[1]));
        assert!(paths.iter().all(|p| !p.exists()));
        assert_eq!(
            fs::read_to_string(&download.path).unwrap(),
            "previous cache"
        );
    }
}

#[test]
fn successful_retry_stops_without_extra_wait_and_cleans_partial() {
    let temp = TemporaryDirectory::new("download-success").unwrap();
    let (plan, _) = syft::prepare(temp.path(), None, &temp.path().join("verified")).unwrap();
    let mut calls = 0;
    let mut waits = Vec::new();
    download_with(
        &plan.downloads[0],
        temp.path(),
        |path| {
            calls += 1;
            fs::write(path, "transport bytes").unwrap();
            if calls == 1 {
                Err("retry".into())
            } else {
                Ok(())
            }
        },
        |seconds| waits.push(seconds),
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(waits, [1]);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}
