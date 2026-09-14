"""主窗口 ViewModel：承载纯 UI 状态，不依赖界面控件。"""

from __future__ import annotations

from PySide6.QtCore import Property, QObject, Signal, Slot


class MainWindowViewModel(QObject):
    """管理导航与主导航组的展开状态。"""

    page_inference = 0
    page_models = 1
    page_preannotation = 2
    page_annotation = 3
    page_pretraining = 4
    page_training = 5
    page_settings = 6
    page_about = 7

    currentPageChanged = Signal()
    modelsExpandedChanged = Signal()

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._current_page = self.page_inference
        self._models_expanded = True

    @Property(int, notify=currentPageChanged)
    def currentPage(self) -> int:
        return self._current_page

    @Slot(int, result=int)
    def requestPage(self, page: int) -> int:
        if page not in {
            self.page_inference,
            self.page_models,
            self.page_preannotation,
            self.page_annotation,
            self.page_pretraining,
            self.page_training,
            self.page_settings,
            self.page_about,
        }:
            return self._current_page
        if page != self._current_page:
            self._current_page = page
            self.currentPageChanged.emit()
        return self._current_page

    @Property(bool, notify=modelsExpandedChanged)
    def modelsExpanded(self) -> bool:
        return self._models_expanded

    @Slot()
    def toggleModels(self) -> None:
        self._models_expanded = not self._models_expanded
        self.modelsExpandedChanged.emit()
