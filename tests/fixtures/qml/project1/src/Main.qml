import QtQuick 2.15
import QtQuick.Controls 2.15

ApplicationWindow {
    id: root
    width: 640
    height: 480
    visible: true
    title: qsTr("Probe Fixture")

    property int clickCount: 0
    property string statusText: "idle"

    signal resetRequested()

    function incrementCount() {
        clickCount += 1
        statusText = "clicked"
    }

    function resetCount() {
        clickCount = 0
        statusText = "idle"
        resetRequested()
    }

    RequirementCard {
        id: card
        anchors.centerIn: parent
        reqId: "REQ-001"
        onRequirementActivated: function(reqId) {
            root.incrementCount()
        }
    }
}
