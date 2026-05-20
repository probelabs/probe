// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IVotesLike {
    function getVotes(address account) external view returns (uint256);
}

library VoteMath {
    function quorum(uint256 supply, uint256 numerator) internal pure returns (uint256) {
        return (supply * numerator) / 100;
    }
}

contract GovernorExample {
    enum ProposalState {
        Pending,
        Active,
        Succeeded,
        Defeated
    }

    struct ProposalCore {
        uint64 voteStart;
        uint64 voteEnd;
        bool executed;
    }

    event ProposalCreated(uint256 indexed proposalId, address indexed proposer);
    error GovernorUnexpectedProposalState(uint256 proposalId, ProposalState current);

    IVotesLike public immutable token;
    mapping(uint256 => ProposalCore) private _proposals;

    modifier onlyActive(uint256 proposalId) {
        if (state(proposalId) != ProposalState.Active) {
            revert GovernorUnexpectedProposalState(proposalId, state(proposalId));
        }
        _;
    }

    constructor(IVotesLike tokenAddress) {
        token = tokenAddress;
    }

    function propose(address[] memory targets, bytes[] memory calldatas) public returns (uint256) {
        uint256 proposalId = hashProposal(targets, calldatas);
        _proposals[proposalId] = ProposalCore({
            voteStart: uint64(block.number + votingDelay()),
            voteEnd: uint64(block.number + votingDelay() + votingPeriod()),
            executed: false
        });
        emit ProposalCreated(proposalId, msg.sender);
        return proposalId;
    }

    function castVote(uint256 proposalId, uint8 support) public onlyActive(proposalId) returns (uint256) {
        return token.getVotes(msg.sender) + support;
    }

    function state(uint256 proposalId) public view returns (ProposalState) {
        ProposalCore storage proposal = _proposals[proposalId];
        if (proposal.executed) {
            return ProposalState.Succeeded;
        }
        if (block.number < proposal.voteStart) {
            return ProposalState.Pending;
        }
        if (block.number <= proposal.voteEnd) {
            return ProposalState.Active;
        }
        return ProposalState.Defeated;
    }

    function hashProposal(address[] memory targets, bytes[] memory calldatas) public pure returns (uint256) {
        return uint256(keccak256(abi.encode(targets, calldatas)));
    }

    function votingDelay() public pure returns (uint256) {
        return 1;
    }

    function votingPeriod() public pure returns (uint256) {
        return 45818;
    }
}
