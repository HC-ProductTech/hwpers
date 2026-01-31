use std::path::PathBuf;
use std::process::Command;

fn cargo_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("jsontohwpx");
    path
}

fn examples_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/jsontohwpx")
}

fn simple_json() -> PathBuf {
    examples_path().join("simple_text.json")
}

#[test]
fn test_cli_basic_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("out.hwpx");

    let status = Command::new(cargo_bin())
        .arg(simple_json())
        .arg("-o")
        .arg(&output)
        .arg("-b")
        .arg(examples_path())
        .status()
        .unwrap();

    assert!(status.success());
    assert!(output.exists());
    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.len() > 100);
    assert_eq!(&bytes[0..2], &[0x50, 0x4B]); // ZIP magic
}

#[test]
fn test_cli_auto_output_filename() {
    let tmp = tempfile::tempdir().unwrap();

    let status = Command::new(cargo_bin())
        .arg(simple_json())
        .arg("-b")
        .arg(examples_path())
        .current_dir(tmp.path())
        .status()
        .unwrap();

    assert!(status.success());
    // simple_text.json의 article_id를 확인하여 자동 파일명 검증
    let json = std::fs::read_to_string(simple_json()).unwrap();
    let input: hwpers::jsontohwpx::ArticleDocument = serde_json::from_str(&json).unwrap();
    let expected_file = tmp.path().join(format!("{}.hwpx", input.article_id));
    assert!(
        expected_file.exists(),
        "자동 생성된 파일 없음: {}",
        expected_file.display()
    );
}

#[test]
fn test_cli_validate_success() {
    let output = Command::new(cargo_bin())
        .arg(simple_json())
        .arg("--validate")
        .arg("-b")
        .arg(examples_path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("검증 성공"), "stderr: {}", stderr);
}

#[test]
fn test_cli_validate_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let bad_json = tmp.path().join("bad.json");
    std::fs::write(&bad_json, r#"{"article_id":"  ","title":"S"}"#).unwrap();

    let output = Command::new(cargo_bin())
        .arg(&bad_json)
        .arg("--validate")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn test_cli_include_header() {
    let tmp = tempfile::tempdir().unwrap();
    let output_file = tmp.path().join("header.hwpx");

    let status = Command::new(cargo_bin())
        .arg(simple_json())
        .arg("-o")
        .arg(&output_file)
        .arg("-b")
        .arg(examples_path())
        .arg("--include-header")
        .status()
        .unwrap();

    assert!(status.success());
    let bytes = std::fs::read(&output_file).unwrap();
    let doc = hwpers::HwpxReader::from_bytes(&bytes).unwrap();
    let text = doc.extract_text();
    assert!(
        text.contains("제목:"),
        "헤더에 '제목:' 포함되어야 함: {}",
        text
    );
}

#[test]
fn test_cli_stdin_input() {
    let json = std::fs::read_to_string(simple_json()).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("stdin_out.hwpx");

    let mut child = Command::new(cargo_bin())
        .arg("-")
        .arg("-o")
        .arg(&output)
        .arg("-b")
        .arg(examples_path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
    let status = child.wait().unwrap();

    assert!(status.success());
    assert!(output.exists());
}

#[test]
fn test_cli_nonexistent_file() {
    let output = Command::new(cargo_bin())
        .arg("/nonexistent/path/file.json")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn test_cli_invalid_json() {
    let tmp = tempfile::tempdir().unwrap();
    let bad_file = tmp.path().join("invalid.json");
    std::fs::write(&bad_file, "{ not valid json }").unwrap();

    let output = Command::new(cargo_bin()).arg(&bad_file).output().unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn test_cli_json_error_format() {
    let output = Command::new(cargo_bin())
        .arg("/nonexistent.json")
        .arg("--json")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(r#""error""#),
        "JSON error format: {}",
        stderr
    );
    assert!(
        stderr.contains(r#""code":1"#),
        "JSON code field: {}",
        stderr
    );
}

#[test]
fn test_cli_progress_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let output_file = tmp.path().join("progress.hwpx");

    let output = Command::new(cargo_bin())
        .arg(simple_json())
        .arg("-o")
        .arg(&output_file)
        .arg("-b")
        .arg(examples_path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[1/3]"), "progress step 1: {}", stderr);
    assert!(stderr.contains("[2/3]"), "progress step 2: {}", stderr);
    assert!(stderr.contains("[3/3]"), "progress step 3: {}", stderr);
    assert!(
        stderr.contains("변환 완료"),
        "completion message: {}",
        stderr
    );
}

#[test]
fn test_cli_validate_progress_logs() {
    let output = Command::new(cargo_bin())
        .arg(simple_json())
        .arg("--validate")
        .arg("-b")
        .arg(examples_path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[1/2]"), "validate step 1: {}", stderr);
    assert!(stderr.contains("[2/2]"), "validate step 2: {}", stderr);
}

#[test]
fn test_cli_empty_table_conversion_error() {
    let tmp = tempfile::tempdir().unwrap();
    let json_file = tmp.path().join("empty_table.json");
    std::fs::write(&json_file, r#"{"article_id":"ERR001","title":"S","contents":[{"type":"table","value":"<table></table>"}]}"#).unwrap();

    let output = Command::new(cargo_bin()).arg(&json_file).output().unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2)); // Conversion error
}
