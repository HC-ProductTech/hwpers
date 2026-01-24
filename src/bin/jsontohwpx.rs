use std::path::PathBuf;
use std::process;

use clap::Parser;

use hwpers::jsontohwpx::{self, ApiResponse, JsonToHwpxError};

#[derive(Parser)]
#[command(name = "jsontohwpx", about = "JSON API 응답을 HWPX 문서로 변환")]
struct Cli {
    /// 입력 JSON 파일 경로
    #[arg(short, long)]
    input: PathBuf,

    /// 출력 HWPX 파일 경로
    #[arg(short, long)]
    output: PathBuf,

    /// 이미지 기본 경로 (상대 경로 이미지 해석용)
    #[arg(short, long, default_value = ".")]
    base_path: PathBuf,

    /// JSON 형식으로 에러 출력
    #[arg(long, default_value_t = false)]
    json: bool,
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
    let json_str = std::fs::read_to_string(&cli.input).map_err(|e| {
        JsonToHwpxError::Input(format!(
            "입력 파일 읽기 실패: {} ({})",
            cli.input.display(),
            e
        ))
    })?;

    let input: ApiResponse = serde_json::from_str(&json_str)
        .map_err(|e| JsonToHwpxError::Input(format!("JSON 파싱 실패: {}", e)))?;

    jsontohwpx::convert_to_file(&input, &cli.base_path, &cli.output)?;

    println!("변환 완료: {}", cli.output.display());
    Ok(())
}

fn exit_code(e: &JsonToHwpxError) -> i32 {
    match e {
        JsonToHwpxError::Input(_) => 1,
        JsonToHwpxError::Conversion(_) => 2,
        JsonToHwpxError::Io(_) => 3,
        JsonToHwpxError::Hwpx(_) => 2,
    }
}
