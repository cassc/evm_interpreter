
// SPDX-License-Identifier: MIT
pragma solidity 0.7.6;
pragma experimental ABIEncoderV2;

contract Test {
  struct Sig { uint8 v; bytes32 r; bytes32 s;}

  function claim(bytes32 _msg, Sig memory sig) public {
    address signer = ecrecover(_msg, sig.v, sig.r, sig.s);
    // require(signer == owner);
    payable(msg.sender).transfer(address(this).balance);
  }

  function vec_add(uint[2] memory a, uint[2] memory b) public returns (uint[2] memory c){
    c[0] = a[0] + b[0]; //overflow
    c[1] = a[1] + b[1]; //overflow
  }
}

contract BugSample {
    uint256 uzero = 0;
    uint256 umax = 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff;
    uint256 guards = 1;
    address winner;
    address owner;
    bool reentrancy_guard = false;

    constructor() {
        owner = msg.sender;
    }

    function overflow_add(uint256 b) public view returns (uint256) {
        return umax + b; // OVERFLOW
    }

    function underflow_minus(uint256 b) public view returns (uint256) {
        return uzero - b; // UNDERFLOW
    }

    function div_by_zero(uint256 m) public view returns (uint256) {
        return m / uzero; // DIVISION_BY_ZERO
        // Not a bug (always revert after solidity 0.4.0)
    }

    function reentrancy_withdraw() public {
        require(guards > 0, "Must have some guards left");
        (bool success, ) = payable(msg.sender).call{value: 1 ether}(""); // REENTRANCY, UNCHECKED_SEND
        require(success);
        guards = 0;
    }

    function access_control(uint256 _guards, address addr) public {
        // POSSIBLE_ACCESS_CONTROL_BUG
        require(reentrancy_guard == false);
        reentrancy_guard = true;
        (bool success, ) = address(addr).call("");
        require(success);
        guards = _guards;
        reentrancy_guard = false;
    }

    function assert_failure(uint256 _umax) public payable {
        assert(_umax > 0); // ASSERTION_FAILURE
        umax = _umax;
    }

    function guess(uint256 i) public payable {
        require(msg.value > 0, "Must pay to play");

        if (block.timestamp % i == 89) {
            // TIME_STAMP
            winner = msg.sender;
        }

        if (block.number % i == 97) {
            // BLOCK_NUMBER
            winner = msg.sender;
        }
    }

    function exception(address addr) public {
        address(addr).call("0x1234"); // EXCEPTION_DISORDER, ADDRESS_VALIDATION
    }

    function kill() public {
        selfdestruct(msg.sender); // UNPROTECTED_SELFDESTRUCT
    }
    bool lock1;
    bool lock2;
    function unlock1 () public {
      lock1 = true;
    }
    function unlock2 () public {
      lock2 = true;
    }
    function three_step_bug() public {
        require(lock1);
        require(lock2);
      selfdestruct(msg.sender);
    }
}
