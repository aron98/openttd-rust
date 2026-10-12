local d={["α\0β"]=9};
 local t={};
 delegate d:t;
 assert(t["α\0β"]==9);
 return 1;

