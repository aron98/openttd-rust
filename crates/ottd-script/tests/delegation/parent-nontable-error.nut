local a=[];
 local caught=false;
 try{local p=a.parent;
}catch(e){caught=true;
 assert(e=="the array type doesn't have a parent slot");
} assert(caught);
 return 1;

