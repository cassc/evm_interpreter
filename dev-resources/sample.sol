// SPDX-License-Identifier: GPL-3.0

pragma solidity >0.8.0;


contract A{
    uint public ai;
    address public me;
    function init()  public{
        require(me == address(0), "you can only init once");
        me = address(this);
    }
    function changeSomething(int256 i) public returns (uint256){
        ai = uint256(i + 0);
        return ai;
    }


}
