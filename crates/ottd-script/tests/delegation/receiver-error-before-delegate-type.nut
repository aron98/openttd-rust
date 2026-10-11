local caught=false;
 try{delegate 2:3;
}catch(e){caught=true;
 assert(e=="delegating a 'integer'");
} assert(caught);
 return 1;

