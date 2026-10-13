local t={x=7};
assert((false && "x" in t)==false);
assert(true && "x" in t);
return 1;

