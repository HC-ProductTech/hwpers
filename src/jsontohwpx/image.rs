use std::path::Path;

use crate::hwpx::{HwpxImage, HwpxWriter};

use super::error::{JsonToHwpxError, Result};

/// 이미지 URL/경로에서 이미지를 로드하여 HwpxWriter에 추가
pub fn add_image_from_url(writer: &mut HwpxWriter, url: &str, base_path: &Path) -> Result<()> {
    let image_bytes = load_image_bytes(url, base_path)?;
    let image_bytes = convert_if_needed(image_bytes, url)?;

    let image = HwpxImage::from_bytes(image_bytes).ok_or_else(|| {
        JsonToHwpxError::Conversion(format!("지원하지 않는 이미지 포맷: {}", url))
    })?;

    writer.add_image(image)?;
    Ok(())
}

/// Base64 인코딩된 이미지를 디코딩하여 HwpxWriter에 추가
pub fn add_image_from_base64(
    writer: &mut HwpxWriter,
    data: &str,
    format: Option<&str>,
) -> Result<()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| JsonToHwpxError::Conversion(format!("Base64 디코딩 실패: {}", e)))?;

    let bytes = convert_if_needed_by_format(bytes, format)?;

    let image = HwpxImage::from_bytes(bytes)
        .ok_or_else(|| JsonToHwpxError::Conversion("Base64 이미지 포맷 인식 실패".to_string()))?;

    writer.add_image(image)?;
    Ok(())
}

/// URL 또는 로컬 경로에서 이미지 바이트를 로드
fn load_image_bytes(url: &str, base_path: &Path) -> Result<Vec<u8>> {
    if url.starts_with("http://") || url.starts_with("https://") {
        download_image(url)
    } else {
        let path = base_path.join(url);
        std::fs::read(&path).map_err(|e| {
            JsonToHwpxError::Conversion(format!(
                "이미지 파일 읽기 실패: {} ({})",
                path.display(),
                e
            ))
        })
    }
}

/// 외부 URL에서 이미지 다운로드 (타임아웃 60초)
fn download_image(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| JsonToHwpxError::Conversion(format!("HTTP 클라이언트 생성 실패: {}", e)))?;

    let response = client.get(url).send().map_err(|e| {
        JsonToHwpxError::Conversion(format!("이미지 다운로드 실패: {} ({})", url, e))
    })?;

    if !response.status().is_success() {
        return Err(JsonToHwpxError::Conversion(format!(
            "이미지 다운로드 실패: {} (HTTP {})",
            url,
            response.status()
        )));
    }

    response.bytes().map(|b| b.to_vec()).map_err(|e| {
        JsonToHwpxError::Conversion(format!("이미지 데이터 읽기 실패: {} ({})", url, e))
    })
}

/// 확장자 기반으로 변환이 필요한 포맷인지 확인 후 PNG로 변환
fn convert_if_needed(bytes: Vec<u8>, url: &str) -> Result<Vec<u8>> {
    let ext = Path::new(url)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("webp") | Some("avif") => convert_to_png(&bytes),
        Some("gif") => extract_gif_first_frame(&bytes),
        _ => {
            // 매직 바이트로 추가 확인
            if is_webp(&bytes) || is_avif(&bytes) {
                convert_to_png(&bytes)
            } else if is_gif(&bytes) {
                extract_gif_first_frame(&bytes)
            } else {
                Ok(bytes)
            }
        }
    }
}

/// format 필드 기반으로 변환 필요 여부 확인
fn convert_if_needed_by_format(bytes: Vec<u8>, format: Option<&str>) -> Result<Vec<u8>> {
    match format.map(|f| f.to_lowercase()).as_deref() {
        Some("webp") | Some("avif") => convert_to_png(&bytes),
        Some("gif") => extract_gif_first_frame(&bytes),
        _ => Ok(bytes),
    }
}

/// image 크레이트를 사용하여 PNG로 변환
fn convert_to_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| JsonToHwpxError::Conversion(format!("이미지 포맷 변환 실패: {}", e)))?;

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| JsonToHwpxError::Conversion(format!("PNG 인코딩 실패: {}", e)))?;

    Ok(png_bytes)
}

/// GIF에서 첫 프레임 추출 → PNG로 변환
fn extract_gif_first_frame(bytes: &[u8]) -> Result<Vec<u8>> {
    // image 크레이트가 GIF 첫 프레임을 자동으로 로드
    convert_to_png(bytes)
}

/// WebP 매직 바이트 확인
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// AVIF 매직 바이트 확인 (ftyp box)
fn is_avif(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[4..8] == b"ftyp" && &bytes[8..12] == b"avif"
}

/// GIF 매직 바이트 확인 (GIF87a 또는 GIF89a)
fn is_gif(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && &bytes[0..3] == b"GIF" && (&bytes[3..6] == b"87a" || &bytes[3..6] == b"89a")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn examples_path() -> PathBuf {
        PathBuf::from("examples/jsontohwpx")
    }

    // --- 매직 바이트 감지 테스트 ---

    #[test]
    fn test_is_webp() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        assert!(is_webp(&data));
    }

    #[test]
    fn test_is_not_webp() {
        let data = vec![0u8; 12];
        assert!(!is_webp(&data));
    }

    #[test]
    fn test_is_avif() {
        let mut data = vec![0u8; 12];
        data[4..8].copy_from_slice(b"ftyp");
        data[8..12].copy_from_slice(b"avif");
        assert!(is_avif(&data));
    }

    #[test]
    fn test_is_gif() {
        let data = b"GIF89a\x00\x00\x00\x00\x00\x00";
        assert!(is_gif(data));
    }

    #[test]
    fn test_is_gif87a() {
        let data = b"GIF87a\x00\x00\x00\x00\x00\x00";
        assert!(is_gif(data));
    }

    #[test]
    fn test_is_not_gif() {
        let data = b"PNG\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert!(!is_gif(data));
    }

    #[test]
    fn test_png_magic_bytes() {
        // PNG 시그니처: 89 50 4E 47 0D 0A 1A 0A
        let data: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        assert!(!is_webp(&data));
        assert!(!is_avif(&data));
        assert!(!is_gif(&data));
    }

    #[test]
    fn test_jpeg_magic_bytes() {
        // JPEG 시그니처: FF D8 FF
        let data: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(!is_webp(&data));
        assert!(!is_avif(&data));
        assert!(!is_gif(&data));
    }

    // --- 변환 로직 테스트 ---

    #[test]
    fn test_convert_if_needed_png_passthrough() {
        let png_bytes = std::fs::read(examples_path().join("test_img.png")).unwrap();
        let result = convert_if_needed(png_bytes.clone(), "image.png").unwrap();
        // PNG는 변환 없이 그대로 통과
        assert_eq!(result, png_bytes);
    }

    #[test]
    fn test_convert_if_needed_jpeg_passthrough() {
        let jpg_bytes = std::fs::read(examples_path().join("test_img.jpg")).unwrap();
        let result = convert_if_needed(jpg_bytes.clone(), "photo.jpg").unwrap();
        assert_eq!(result, jpg_bytes);
    }

    #[test]
    fn test_convert_if_needed_jpeg_extension() {
        let jpeg_bytes = std::fs::read(examples_path().join("test_img.jpeg")).unwrap();
        let result = convert_if_needed(jpeg_bytes.clone(), "photo.jpeg").unwrap();
        assert_eq!(result, jpeg_bytes);
    }

    #[test]
    fn test_convert_if_needed_webp_to_png() {
        let webp_bytes = std::fs::read(examples_path().join("test_img.webp")).unwrap();
        let result = convert_if_needed(webp_bytes, "image.webp").unwrap();
        // 결과는 PNG 시그니처를 가져야 함
        assert_eq!(&result[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_convert_if_needed_gif_first_frame() {
        let gif_bytes = std::fs::read(examples_path().join("test_img.gif")).unwrap();
        let result = convert_if_needed(gif_bytes, "animation.gif").unwrap();
        // GIF는 첫 프레임 추출 후 PNG로 변환
        assert_eq!(&result[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_convert_if_needed_webp_magic_bytes_no_extension() {
        let webp_bytes = std::fs::read(examples_path().join("test_img.webp")).unwrap();
        // 확장자 없이 매직 바이트로 감지
        let result = convert_if_needed(webp_bytes, "image_no_ext").unwrap();
        assert_eq!(&result[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_convert_if_needed_gif_magic_bytes_no_extension() {
        let gif_bytes = std::fs::read(examples_path().join("test_img.gif")).unwrap();
        // 확장자 없이 매직 바이트로 GIF 감지
        let result = convert_if_needed(gif_bytes, "image_no_ext").unwrap();
        assert_eq!(&result[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    // --- Base64 디코딩 테스트 ---

    #[test]
    fn test_base64_decode_png() {
        // 1x1 투명 PNG의 Base64
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let mut writer = HwpxWriter::new();
        add_image_from_base64(&mut writer, b64, Some("png")).unwrap();
        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_base64_invalid_data() {
        let mut writer = HwpxWriter::new();
        let result = add_image_from_base64(&mut writer, "!!!invalid!!!", Some("png"));
        assert!(result.is_err());
    }

    // --- 로컬 파일 로딩 테스트 ---

    #[test]
    fn test_load_local_png() {
        let mut writer = HwpxWriter::new();
        add_image_from_url(&mut writer, "./test_img.png", &examples_path()).unwrap();
        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_load_local_jpg() {
        let mut writer = HwpxWriter::new();
        add_image_from_url(&mut writer, "./test_img.jpg", &examples_path()).unwrap();
        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_load_local_webp_converts_to_png() {
        let mut writer = HwpxWriter::new();
        add_image_from_url(&mut writer, "./test_img.webp", &examples_path()).unwrap();
        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_load_local_gif_extracts_first_frame() {
        let mut writer = HwpxWriter::new();
        add_image_from_url(&mut writer, "./test_img.gif", &examples_path()).unwrap();
        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_load_nonexistent_file_fails() {
        let mut writer = HwpxWriter::new();
        let result = add_image_from_url(&mut writer, "./nonexistent.png", &examples_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_relative_path_resolution() {
        let base = PathBuf::from("examples/jsontohwpx");
        let bytes = load_image_bytes("./test_img.png", &base).unwrap();
        // PNG 시그니처 확인
        assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }
}
