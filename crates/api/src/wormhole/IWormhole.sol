// contracts/Bridge.sol
// SPDX-License-Identifier: Apache 2

pragma solidity ^0.8.0;

interface IERC20 {
    function totalSupply() external view returns (uint256);

    function balanceOf(address who) external view returns (uint256);

    function allowance(
        address owner,
        address spender
    ) external view returns (uint256);

    function transfer(address to, uint256 value) external returns (bool);

    function approve(address spender, uint256 value) external returns (bool);

    function transferFrom(
        address from,
        address to,
        uint256 value
    ) external returns (bool);

    // From ERC20Detailed
    function name() external view returns (string memory);
    function symbol() external view returns (string memory);
    function decimals() external view returns (uint8);

    // Events
    event Transfer(address indexed from, address indexed to, uint256 value);

    event Approval(
        address indexed owner,
        address indexed spender,
        uint256 value
    );
}

interface IWETH is IERC20 {
    function deposit() external payable;
    function withdraw(uint amount) external;
}

interface IWormhole {
    struct GuardianSet {
        address[] keys;
        uint32 expirationTime;
    }

    struct Signature {
        bytes32 r;
        bytes32 s;
        uint8 v;
        uint8 guardianIndex;
    }

    struct VM {
        uint8 version;
        uint32 timestamp;
        uint32 nonce;
        uint16 emitterChainId;
        bytes32 emitterAddress;
        uint64 sequence;
        uint8 consistencyLevel;
        bytes payload;
        uint32 guardianSetIndex;
        Signature[] signatures;
        bytes32 hash;
    }

    struct ContractUpgrade {
        bytes32 module;
        uint8 action;
        uint16 chain;
        address newContract;
    }

    struct GuardianSetUpgrade {
        bytes32 module;
        uint8 action;
        uint16 chain;
        GuardianSet newGuardianSet;
        uint32 newGuardianSetIndex;
    }

    struct SetMessageFee {
        bytes32 module;
        uint8 action;
        uint16 chain;
        uint256 messageFee;
    }

    struct TransferFees {
        bytes32 module;
        uint8 action;
        uint16 chain;
        uint256 amount;
        bytes32 recipient;
    }

    struct RecoverChainId {
        bytes32 module;
        uint8 action;
        uint256 evmChainId;
        uint16 newChainId;
    }

    event LogMessagePublished(
        address indexed sender,
        uint64 sequence,
        uint32 nonce,
        bytes payload,
        uint8 consistencyLevel
    );
    event ContractUpgraded(
        address indexed oldContract,
        address indexed newContract
    );
    event GuardianSetAdded(uint32 indexed index);

    function publishMessage(
        uint32 nonce,
        bytes memory payload,
        uint8 consistencyLevel
    ) external payable returns (uint64 sequence);

    function initialize() external;

    function parseAndVerifyVM(
        bytes calldata encodedVM
    ) external view returns (VM memory vm, bool valid, string memory reason);

    function verifyVM(
        VM memory vm
    ) external view returns (bool valid, string memory reason);

    function verifySignatures(
        bytes32 hash,
        Signature[] memory signatures,
        GuardianSet memory guardianSet
    ) external pure returns (bool valid, string memory reason);

    function parseVM(
        bytes memory encodedVM
    ) external pure returns (VM memory vm);

    function quorum(
        uint numGuardians
    ) external pure returns (uint numSignaturesRequiredForQuorum);

    function getGuardianSet(
        uint32 index
    ) external view returns (GuardianSet memory);

    function getCurrentGuardianSetIndex() external view returns (uint32);

    function getGuardianSetExpiry() external view returns (uint32);

    function governanceActionIsConsumed(
        bytes32 hash
    ) external view returns (bool);

    function isInitialized(address impl) external view returns (bool);

    function chainId() external view returns (uint16);

    function isFork() external view returns (bool);

    function governanceChainId() external view returns (uint16);

    function governanceContract() external view returns (bytes32);

    function messageFee() external view returns (uint256);

    function evmChainId() external view returns (uint256);

    function nextSequence(address emitter) external view returns (uint64);

    function parseContractUpgrade(
        bytes memory encodedUpgrade
    ) external pure returns (ContractUpgrade memory cu);

    function parseGuardianSetUpgrade(
        bytes memory encodedUpgrade
    ) external pure returns (GuardianSetUpgrade memory gsu);

    function parseSetMessageFee(
        bytes memory encodedSetMessageFee
    ) external pure returns (SetMessageFee memory smf);

    function parseTransferFees(
        bytes memory encodedTransferFees
    ) external pure returns (TransferFees memory tf);

    function parseRecoverChainId(
        bytes memory encodedRecoverChainId
    ) external pure returns (RecoverChainId memory rci);

    function submitContractUpgrade(bytes memory _vm) external;

    function submitSetMessageFee(bytes memory _vm) external;

    function submitNewGuardianSet(bytes memory _vm) external;

    function submitTransferFees(bytes memory _vm) external;

    function submitRecoverChainId(bytes memory _vm) external;
}

interface ITokenBridge {
    struct Transfer {
        uint8 payloadID;
        uint256 amount;
        bytes32 tokenAddress;
        uint16 tokenChain;
        bytes32 to;
        uint16 toChain;
        uint256 fee;
    }

    struct TransferWithPayload {
        uint8 payloadID;
        uint256 amount;
        bytes32 tokenAddress;
        uint16 tokenChain;
        bytes32 to;
        uint16 toChain;
        bytes32 fromAddress;
        bytes payload;
    }

    struct AssetMeta {
        uint8 payloadID;
        bytes32 tokenAddress;
        uint16 tokenChain;
        uint8 decimals;
        bytes32 symbol;
        bytes32 name;
    }

    struct RegisterChain {
        bytes32 module;
        uint8 action;
        uint16 chainId;
        uint16 emitterChainID;
        bytes32 emitterAddress;
    }

    struct UpgradeContract {
        bytes32 module;
        uint8 action;
        uint16 chainId;
        bytes32 newContract;
    }

    struct RecoverChainId {
        bytes32 module;
        uint8 action;
        uint256 evmChainId;
        uint16 newChainId;
    }

    event ContractUpgraded(
        address indexed oldContract,
        address indexed newContract
    );

    event TransferRedeemed(
        uint16 indexed emitterChainId,
        bytes32 indexed emitterAddress,
        uint64 indexed sequence
    );

    function _parseTransferCommon(
        bytes memory encoded
    ) external pure returns (Transfer memory transfer);

    function attestToken(
        address tokenAddress,
        uint32 nonce
    ) external payable returns (uint64 sequence);

    function wrapAndTransferETH(
        uint16 recipientChain,
        bytes32 recipient,
        uint256 arbiterFee,
        uint32 nonce
    ) external payable returns (uint64 sequence);

    function wrapAndTransferETHWithPayload(
        uint16 recipientChain,
        bytes32 recipient,
        uint32 nonce,
        bytes memory payload
    ) external payable returns (uint64 sequence);

    function transferTokens(
        address token,
        uint256 amount,
        uint16 recipientChain,
        bytes32 recipient,
        uint256 arbiterFee,
        uint32 nonce
    ) external payable returns (uint64 sequence);

    function transferTokensWithPayload(
        address token,
        uint256 amount,
        uint16 recipientChain,
        bytes32 recipient,
        uint32 nonce,
        bytes memory payload
    ) external payable returns (uint64 sequence);

    function updateWrapped(
        bytes memory encodedVm
    ) external returns (address token);

    function createWrapped(
        bytes memory encodedVm
    ) external returns (address token);

    function completeTransferWithPayload(
        bytes memory encodedVm
    ) external returns (bytes memory);

    function completeTransferAndUnwrapETHWithPayload(
        bytes memory encodedVm
    ) external returns (bytes memory);

    function completeTransfer(bytes memory encodedVm) external;

    function completeTransferAndUnwrapETH(bytes memory encodedVm) external;

    function encodeAssetMeta(
        AssetMeta memory meta
    ) external pure returns (bytes memory encoded);

    function encodeTransfer(
        Transfer memory transfer
    ) external pure returns (bytes memory encoded);

    function encodeTransferWithPayload(
        TransferWithPayload memory transfer
    ) external pure returns (bytes memory encoded);

    function parsePayloadID(
        bytes memory encoded
    ) external pure returns (uint8 payloadID);

    function parseAssetMeta(
        bytes memory encoded
    ) external pure returns (AssetMeta memory meta);

    function parseTransfer(
        bytes memory encoded
    ) external pure returns (Transfer memory transfer);

    function parseTransferWithPayload(
        bytes memory encoded
    ) external pure returns (TransferWithPayload memory transfer);

    function governanceActionIsConsumed(
        bytes32 hash
    ) external view returns (bool);

    function isInitialized(address impl) external view returns (bool);

    function isTransferCompleted(bytes32 hash) external view returns (bool);

    function wormhole() external view returns (IWormhole);

    function chainId() external view returns (uint16);

    function evmChainId() external view returns (uint256);

    function isFork() external view returns (bool);

    function governanceChainId() external view returns (uint16);

    function governanceContract() external view returns (bytes32);

    function wrappedAsset(
        uint16 tokenChainId,
        bytes32 tokenAddress
    ) external view returns (address);

    function bridgeContracts(uint16 chainId_) external view returns (bytes32);

    function tokenImplementation() external view returns (address);

    function WETH() external view returns (IWETH);

    function outstandingBridged(address token) external view returns (uint256);

    function isWrappedAsset(address token) external view returns (bool);

    function finality() external view returns (uint8);

    function implementation() external view returns (address);

    function initialize() external;

    function registerChain(bytes memory encodedVM) external;

    function upgrade(bytes memory encodedVM) external;

    function submitRecoverChainId(bytes memory encodedVM) external;

    function parseRegisterChain(
        bytes memory encoded
    ) external pure returns (RegisterChain memory chain);

    function parseUpgrade(
        bytes memory encoded
    ) external pure returns (UpgradeContract memory chain);

    function parseRecoverChainId(
        bytes memory encodedRecoverChainId
    ) external pure returns (RecoverChainId memory rci);
}
