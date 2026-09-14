"""VisionHyperAgent 应用入口。"""

from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QUrl
from PySide6.QtGui import QFontDatabase, QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle

from vision_hyper_agent.viewmodels.main_window import MainWindowViewModel


def main() -> int:
    """启动 PySide6/QML 主窗口。"""
    app = QGuiApplication(sys.argv)
    app.setOrganizationName("VisionHyperAgent")
    app.setApplicationName("VisionHyperAgent")
    QQuickStyle.setStyle("Basic")

    package_dir = Path(__file__).resolve().parent
    font_path = package_dir / "view" / "resources" / "ui" / "fonts" / "NotoSansCJK-Regular.ttc"
    if font_path.is_file():
        font_id = QFontDatabase.addApplicationFont(str(font_path))
        if font_id == -1:
            print(f"警告：无法加载界面字体 {font_path}", file=sys.stderr)

    engine = QQmlApplicationEngine()
    engine.addImportPath(str(package_dir / "view"))
    view_model = MainWindowViewModel(engine)
    engine.rootContext().setContextProperty("mainWindowViewModel", view_model)
    engine.load(QUrl.fromLocalFile(str(package_dir / "view" / "MainWindow.qml")))
    if not engine.rootObjects():
        return 1
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
