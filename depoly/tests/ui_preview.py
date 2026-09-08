"""Render the native UI for visual review; no Rust host or business service required."""

import os
import re
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]
VIEWER = ROOT / 'depoly/tools/slint/bin' / (
    'slint-viewer.exe' if os.name == 'nt' else 'slint-viewer'
)
OUTPUT = ROOT / 'depoly/target/ui-preview/light'


def run(*args):
    result = subprocess.run(
        [str(VIEWER), *map(str, args)], cwd=ROOT,
        check=True, text=True, capture_output=True,
    )
    if result.stderr.strip():
        raise RuntimeError(result.stderr)


def render(name, page, width, height, extra=''):
    source = OUTPUT / f'{name}-{width}.slint'
    source.write_text(
        'import { MainWindow } from "../../../../src/view/MainWindow.slint";\n'
        'import { ViewPage } from "../../../../src/view/ViewTypes.slint";\n'
        'export component Preview inherits MainWindow {\n'
        f'    preferred-width: {width}px; preferred-height: {height}px;\n'
        f'    current-page: ViewPage.{page};\n'
        f'    {extra}\n'
        '}\n', encoding='utf-8',
    )
    image = source.with_suffix('.png')
    run('--screenshot', image, source)
    header = image.read_bytes()[:24]
    assert header[:8] == b'\x89PNG\r\n\x1a\n', image
    assert struct.unpack('>II', header[16:24]) == (width, height), image
    print(f'PASS {name}: {width} x {height}')


def main():
    if not VIEWER.is_file():
        raise SystemExit(f'Project-local Slint Viewer not found: {VIEWER}')
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for source in (ROOT / 'src/view').rglob('*.slint'):
        text = source.read_text(encoding='utf-8')
        components = re.findall(r'^(?:export )?component (\w+)', text, re.MULTILINE)
        assert len(components) <= 1, (source, components)
        if components:
            assert source.stem == components[0], (source, components[0])
        for target in re.findall(r'import\s*\{[^}]+\}\s*from\s*"([^"]+)"', text):
            if target != 'std-widgets.slint':
                assert (source.parent / target).is_file(), (source, target)
        for removed in ['本机工作区', '预览版', '界面预览',
                        '业务功能尚未接入', '本地预览', 'VHA  /  0.1']:
            assert removed not in text, (source, removed)
    chat = (ROOT / 'src/view/components/GlobalChat.slint').read_text(encoding='utf-8')
    assert '清除聊天记录' in chat
    assert 'Agent 连接设置' not in chat
    assert 'text: "未连接"' not in chat
    inference = (ROOT / 'src/view/pages/InferencePage.slint').read_text(encoding='utf-8')
    assert 'label: "网络触发配置"' not in inference
    assert 'text: "未就绪"' not in inference
    run('--check', ROOT / 'src/view/MainWindow.slint')
    for width, height in [(1440, 900), (1120, 720)]:
        for page in ['inference', 'models', 'preannotation', 'annotation', 'pretraining', 'training', 'settings', 'about']:
            render(page, page, width, height)
        render('models-collapsed', 'models', width, height, 'models-expanded: false;')
        render('preannotation-folded', 'preannotation', width, height, 'models-expanded: false;')
        render('annotation-single', 'annotation', width, height, 'annotation-grid: false;')
    render('models-fixture', 'models', 1120, 720,
           'models-directory: "/preview-fixtures/models"; '
           'model-entries: [{ name: "preview-best.onnx", format: "ONNX", '
           'path: "/preview-fixtures/models/preview-best.onnx" }, '
           '{ name: "long-preview-model-name-to-check-ellipsis.pt", '
           'format: "PT", path: "/preview-fixtures/models/nested/long-preview-model.pt" }]; '
           'selected-model: 0;')
    render('inference-model-name', 'inference', 1120, 720,
           'inference-model-name: "preview-best-very-long-model-name.onnx"; source-choice: 2;')
    render('preannotation-draft', 'preannotation', 1120, 720,
           'feature-draft: { target: "测试对象", appearance: "测试用特征描述。", '
           'distinctions: "测试类别区别。", exclusions: "测试排除情况。" };')
    render('notice', 'inference', 1120, 720, 'feedback: "推理功能暂不可用。";')
    render('long-chat', 'inference', 1120, 720,
           'last-message: "' + 'Long message preview. ' * 80 + '";')
    print(f'Renders ready for visual inspection: {OUTPUT}')


if __name__ == '__main__':
    main()
