use std::io::Read;
use std::path::PathBuf;
use std::process;

use clap::Parser;

use hwpers::jsontohwpx::{self, ApiResponse, JsonToHwpxError};

#[derive(Parser)]
#[command(name = "jsontohwpx", about = "JSON API 응답을 HWPX 문서로 변환")]
struct Cli {
    /// 입력 JSON 파일 경로 ('-'이면 stdin에서 읽기)
    input: String,

    /// 출력 HWPX 파일 경로 (미지정 시 {atclId}.hwpx)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// 이미지 기본 경로 (상대 경로 이미지 해석용)
    #[arg(short, long, default_value = ".")]
    base_path: PathBuf,

    /// JSON 형식으로 에러 출력
    #[arg(long)]
    json: bool,

    /// 검증만 수행 (변환하지 않음)
    #[arg(long)]
    validate: bool,

    /// 헤더 포함 강제 (JSON의 includeHeader 무시)
    #[arg(long)]
    include_header: bool,
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(&cli) {
        if cli.json {
            let code = exit_code(&e);
            eprintln!(
                r#"{{"error":"{}","code":{}}}"#,
                e.to_string().replace('"', "\\\""),
                code
            );
            process::exit(code);
        } else {
            eprintln!("오류: {}", e);
            process::exit(exit_code(&e));
        }
    }
}

fn run(cli: &Cli) -> Result<(), JsonToHwpxError> {
    let total_steps = if cli.validate { 2 } else { 3 };

    // Step 1: JSON 읽기 및 파싱
    log_progress(1, total_steps, "JSON 파싱 중...");
    let json_str = read_input(&cli.input)?;

    let mut input: ApiResponse = serde_json::from_str(&json_str)
        .map_err(|e| JsonToHwpxError::Input(format!("JSON 파싱 실패: {}", e)))?;

    // --include-header 플래그 적용
    if cli.include_header {
        input.options.include_header = true;
    }

    // Step 2: 검증
    if cli.validate {
        log_progress(2, total_steps, "검증 중...");
        input.validate()?;
        eprintln!(
            "검증 성공: responseCode={}, atclId={}, contents={}개",
            input.response_code,
            input.data.article.atcl_id,
            input.data.article.contents.len()
        );
        return Ok(());
    }

    // Step 2: 변환
    let content_count = input.data.article.contents.len();
    log_progress(
        2,
        total_steps,
        &format!("변환 중... ({}개 콘텐츠)", content_count),
    );
    let bytes = jsontohwpx::convert(&input, &cli.base_path)?;

    // Step 3: 파일 저장
    let output_path = resolve_output_path(cli, &input)?;
    log_progress(
        3,
        total_steps,
        &format!("파일 저장 중... {}", output_path.display()),
    );
    std::fs::write(&output_path, bytes)?;

    eprintln!("변환 완료: {}", output_path.display());
    Ok(())
}

/// 입력 소스에서 JSON 문자열 읽기
fn read_input(input: &str) -> Result<String, JsonToHwpxError> {
    if input == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| JsonToHwpxError::Input(format!("stdin 읽기 실패: {}", e)))?;
        Ok(buf)
    } else {
        let path = PathBuf::from(input);
        std::fs::read_to_string(&path).map_err(|e| {
            JsonToHwpxError::Input(format!("입력 파일 읽기 실패: {} ({})", path.display(), e))
        })
    }
}

/// 출력 경로 결정: -o 지정 시 해당 경로, 미지정 시 {atclId}.hwpx
fn resolve_output_path(cli: &Cli, input: &ApiResponse) -> Result<PathBuf, JsonToHwpxError> {
    if let Some(ref output) = cli.output {
        Ok(output.clone())
    } else {
        let atcl_id = &input.data.article.atcl_id;
        let filename = format!("{}.hwpx", atcl_id.trim());
        Ok(PathBuf::from(filename))
    }
}

/// stderr에 진행 로그 출력
fn log_progress(step: usize, total: usize, message: &str) {
    eprintln!("[{}/{}] {}", step, total, message);
}

fn exit_code(e: &JsonToHwpxError) -> i32 {
    match e {
        JsonToHwpxError::Input(_) => 1,
        JsonToHwpxError::Conversion(_) => 2,
        JsonToHwpxError::Io(_) => 3,
        JsonToHwpxError::Hwpx(_) => 2,
    }
}
