import QtQuick
import QtQuick.Controls.Basic
import "../ui_items"

Item {
    id: root

    property bool vertical: false
    property real thickness: 12

    implicitWidth: root.thickness
    implicitHeight: root.thickness

    Rectangle {
        anchors.centerIn: parent
        width: root.vertical ? 48 : 3
        height: root.vertical ? 3 : 48
        radius: 2
        color: SplitHandle.pressed ? Theme.accent
               : (SplitHandle.hovered ? Theme.accent : "#526545ce")
        Behavior on color { ColorAnimation { duration: 120 } }
    }
}
