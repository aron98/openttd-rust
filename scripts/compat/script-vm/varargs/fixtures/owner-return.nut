local f=function(...) {
 delete ::held;
 vargv[0].x=9;
 return vargv[0].x;
};
return f(held);
