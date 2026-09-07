use std::process::Command;

#[test]
fn skeleton_binary_starts_and_reports_its_scope() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_vision-hyper-agent")).output()?;

    assert!(output.status.success(), "骨架程序应正常退出");
    assert!(output.stderr.is_empty(), "正常启动不应输出错误信息");

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("VisionHyperAgent"), "启动输出应标识项目");
    assert!(
        stdout.contains("业务功能尚未实现"),
        "启动输出必须明确当前仅为工程骨架"
    );

    Ok(())
}
