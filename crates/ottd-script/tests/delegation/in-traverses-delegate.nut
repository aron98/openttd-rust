local d={x=7};
 local t={};
 delegate d:t;
 assert("x" in t);
 assert(! ("len" in t));
 return 1;

