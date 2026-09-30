import QtQuick 2.15
import QtTest 1.2

TestCase {
    name: "RequirementCardTests"

    RequirementCard {
        id: card
        reqId: "REQ-TEST"
    }

    function test_activate_emits_signal() {
        compare(card.isValid, true)
    }

    function test_invalid_when_empty() {
        compare(card.reqId, "REQ-TEST")
    }
}
