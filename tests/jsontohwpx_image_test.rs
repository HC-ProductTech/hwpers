use std::path::PathBuf;

use hwpers::jsontohwpx::{self, ApiResponse};
use hwpers::HwpxReader;

fn base_path() -> PathBuf {
    PathBuf::from("examples/jsontohwpx")
}

fn verify_hwpx_bytes(bytes: &[u8]) {
    assert!(!bytes.is_empty(), "HWPX 바이트가 비어있음");
    assert!(
        bytes.len() >= 4 && bytes[0..2] == [0x50, 0x4B],
        "유효한 ZIP 파일이 아닙니다"
    );
    HwpxReader::from_bytes(bytes).expect("HwpxReader가 HWPX 파일을 읽지 못했습니다");
}

fn convert_example_file(filename: &str) {
    let path = base_path().join(filename);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("예제 파일 읽기 실패: {} ({})", path.display(), e));
    let input: ApiResponse = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("JSON 파싱 실패: {} ({})", filename, e));
    let bytes = jsontohwpx::convert(&input, &base_path())
        .unwrap_or_else(|e| panic!("변환 실패: {} ({})", filename, e));
    verify_hwpx_bytes(&bytes);
}

#[test]
fn test_image_png() {
    convert_example_file("image_png.json");
}

#[test]
fn test_image_jpg() {
    convert_example_file("image_jpg.json");
}

#[test]
fn test_image_jpeg() {
    convert_example_file("image_jpeg.json");
}

#[test]
fn test_image_gif() {
    convert_example_file("image_gif.json");
}

#[test]
fn test_image_webp() {
    convert_example_file("image_webp.json");
}

#[test]
fn test_image_avif() {
    convert_example_file("image_avif.json");
}

#[test]
fn test_with_image_text_mixed() {
    let path = base_path().join("with_image.json");
    let json = std::fs::read_to_string(&path).unwrap();
    let input: ApiResponse = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let doc = HwpxReader::from_bytes(&bytes).unwrap();
    let text = doc.extract_text();
    assert!(text.contains("이미지가 첨부되어"));
    assert!(text.contains("참고해주세요"));
}

#[test]
fn test_with_image_base64() {
    convert_example_file("with_image_base64.json");
}

#[test]
fn test_image_multi_format() {
    convert_example_file("image_multi_format.json");
}

#[test]
fn test_image_download_failure() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "IMG_ERR001",
                "subject": "이미지 에러",
                "contents": [
                    { "type": "image", "url": "http://invalid.example.test/nonexistent.png" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let result = jsontohwpx::convert(&input, &base_path());
    assert!(result.is_err(), "존재하지 않는 URL이면 에러를 반환해야 함");
}

#[test]
fn test_image_local_file_not_found() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "IMG_ERR002",
                "subject": "이미지 에러",
                "contents": [
                    { "type": "image", "url": "./nonexistent_image.png" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let result = jsontohwpx::convert(&input, &base_path());
    assert!(result.is_err(), "존재하지 않는 파일이면 에러를 반환해야 함");
}
