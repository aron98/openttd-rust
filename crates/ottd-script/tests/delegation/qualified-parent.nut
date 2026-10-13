local d={x=7};
delegate d:this;
assert(::parent==d);
return 1;
