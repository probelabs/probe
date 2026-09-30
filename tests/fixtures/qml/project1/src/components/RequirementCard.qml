import QtQuick 2.15

Item {
    id: card

    property string reqId: ""
    property alias titleText: title.text
    readonly property bool isValid: reqId !== ""

    signal requirementActivated(string reqId)

    function activate() {
        if (isValid) {
            requirementActivated(reqId)
        }
    }

    Text {
        id: title
        text: card.reqId
    }

    MouseArea {
        anchors.fill: parent
        onClicked: card.activate()
    }
}
