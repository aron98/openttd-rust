// Test-only protocol v09: observe VM-owned varargs without materializing an array.
static void varargs_trace(SQVM *vm) {
 std::cout<<"varargs "<<vm->_vargsstack.size()<<' '<<vm->ci->_vargs.base<<' '<<vm->ci->_vargs.size<<'\n';
 for(SQUnsignedInteger n=0;n<vm->_vargsstack.size();n++) {
  std::cout<<"vararg "<<n<<' ';value(vm->_vargsstack[n]);
 }
}
static int varargs_owner(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr key=SQString::Create(_ss(vm),"held"),leaf=SQTable::Create(_ss(vm),0);
 _table(leaf)->NewSlot(SQString::Create(_ss(vm),"x"),SQObjectPtr(SQInteger(1)));
 _table(vm->_roottable)->NewSlot(key,leaf);
 SQObjectPtr watch=_table(leaf)->GetWeakRef(OT_TABLE);leaf.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,source,"varargs-owner",SQFalse))) {
  std::cerr<<_stringval(vm->_lasterror)<<'\n';return 65;
 }
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=0;
 bool ok=vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue);
 for(int step=0;step<1000;step++) {
  auto alive=type(_weakref(watch)->_obj)==OT_TABLE;
  std::cout<<"owner "<<step<<' '<<ok<<' '<<vm->_suspended<<' '<<vm->_ops_till_suspend<<' '<<vm->_vargsstack.size()<<' '<<alive<<' '<<(alive?_table(_weakref(watch)->_obj)->_uiRef:0)<<'\n';
  if(alive){SQObjectPtr x;_table(_weakref(watch)->_obj)->Get(SQString::Create(_ss(vm),"x"),x);std::cout<<"mutation ";value(x);}
  if(!ok){std::cout<<"error ";value(vm->_lasterror);break;}
  if(!vm->_suspended){std::cout<<"returned ";value(result);break;}
  varargs_trace(vm);
  std::cout<<"collected "<<sq_collectgarbage(vm)<<'\n';
  vm->_ops_till_suspend+=2;
  ok=vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
 }
 if(vm->_suspended&&ok)return 70;
 main.Null();result.Null();vm->temp_reg.Null();sq_settop(vm,0);
 _table(vm->_roottable)->Remove(key);
 std::cout<<"released "<<(type(_weakref(watch)->_obj)==OT_TABLE)<<' '<<vm->_vargsstack.size()<<'\n';
 std::cout<<"final collection "<<sq_collectgarbage(vm)<<'\n';
 std::cout<<"final alive "<<(type(_weakref(watch)->_obj)==OT_TABLE)<<'\n';
 return 0;
}
