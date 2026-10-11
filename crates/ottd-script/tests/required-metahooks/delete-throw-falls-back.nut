local d={_delslot=function(k){throw "delete";
}};
 local t={x=9};
 delegate d:t;
 assert((delete t.x)==9);
 assert(! ("x" in t));
 return 1;

