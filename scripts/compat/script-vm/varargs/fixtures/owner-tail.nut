function step(n,...) {
 if(n==0) return vargv[0].x;
 vargv[0].x+=1;
 return step(n-1,vargv[0]);
}
local f=function(...) {
 delete ::held;
 return step(3,vargv[0]);
};
return f(held);
