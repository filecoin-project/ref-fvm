contract revertit {
  function small() public pure {
     revert("blah blah");
  }

  function big() public pure {
     revert("abcdefghijklmnopqrstuvwyxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-blahblahblahblahb-abcdefghijklmnopqrstuvwyxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-blahblahblahblah-abcdefghijklmnopqrstuvwyxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-blahblahblahblah-abcdefghijklmnopqrstuvwyxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-blahblahblah");
  }
}