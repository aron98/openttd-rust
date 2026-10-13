// Passive compiler-table sessions. Original compiler and VM remain unmodified.
#include "src/3rdparty/squirrel/squirrel/sqtable.h"
#include <array>
#include <sstream>

void constant_snapshot(HSQUIRRELVM vm, const std::string &name, const std::string &member) {
 SQObjectPtr key(SQString::Create(_ss(vm),name)), found;
 std::cout<<"lookup "<<name<<' '<<member<<' ';
 if(!_table(_ss(vm)->_consts)->Get(key,found)) {std::cout<<"absent\n";return;}
 if(member!="-") {
  if(type(found)!=OT_TABLE) {std::cout<<"not_enum\n";return;}
  SQObjectPtr member_key(SQString::Create(_ss(vm),member)), scalar;
  if(!_table(found)->Get(member_key,scalar)) {std::cout<<"missing_member\n";return;}
  value(scalar);
 } else if(type(found)==OT_TABLE) std::cout<<"enum\n";
 else value(found);
}
void constant_dump(const SQObjectPtr &closure) {
 auto proto=_funcproto(_closure(closure)->_function);
 std::cout<<"stack "<<proto->_stacksize<<'\n';
 for(SQInteger n=0;n<proto->_nliterals;++n){std::cout<<"literal ";value(proto->_literals[n]);}
 for(SQInteger n=0;n<proto->_ninstructions;++n){const auto &i=proto->_instructions[n];std::cout<<"op "<<unsigned(i.op)<<' '<<unsigned(i._arg0)<<' '<<i._arg1<<' '<<unsigned(i._arg2)<<' '<<unsigned(i._arg3)<<'\n';}
}
int constants_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream commands(argv[2]);if(!commands)return 66;
 auto root=sq_open(256);auto independent=sq_open(256);
 int status=0;
 {
  auto child=sq_newthread(root,256);SQObjectPtr child_owner=stack_get(root,-1);sq_pop(root,1);
  std::array<HSQUIRRELVM,3> realms={root,child,independent};
  std::array<SQObjectPtr,16> closures;
  std::array<int,16> origins;origins.fill(-1);
  try {
   std::string line;size_t step=0;
   while(std::getline(commands,line)) {
    if(line.empty()||line[0]=='#')continue;
    std::istringstream command(line);std::string op;int realm;
    if(!(command>>op>>realm)||realm<0||realm>=3)throw std::runtime_error("invalid realm command");
    auto vm=realms[realm];if(vm==nullptr)throw std::runtime_error("released realm");std::cout<<"step "<<step++<<' '<<line<<'\n';
    if(op=="compile") {
     size_t slot;std::string mode,path;
     if(!(command>>slot>>mode>>path)||slot>=closures.size()||type(closures[slot])!=OT_NULL)throw std::runtime_error("invalid compile slot");
     std::ifstream input(path,std::ios::binary);if(!input)throw std::runtime_error("missing source");
     std::string source((std::istreambuf_iterator<char>(input)),{});StringConsumer reader(source);
     SQRESULT result;
     if(mode=="buffer")result=sq_compilebuffer(vm,source,"constant-fixture",SQFalse);
     else if(mode=="unsigned"||mode=="utf8")result=sq_compile(vm,mode=="unsigned"?unsigned_feed:utf8_feed,&reader,"constant-fixture",SQFalse);
     else throw std::runtime_error("invalid feed");
     if(SQ_FAILED(result)){std::cout<<"compile_error ";value(vm->_lasterror);}
     else {std::cout<<"compiled\n";closures[slot]=stack_get(vm,-1);origins[slot]=realm;sq_pop(vm,1);constant_dump(closures[slot]);}
    } else if(op=="lookup") {
     std::string name,member;if(!(command>>name>>member))throw std::runtime_error("invalid lookup");
     constant_snapshot(vm,name,member);
    } else if(op=="refs") {
     std::string payload;if(!(command>>payload))throw std::runtime_error("invalid refs");
     SQObjectPtr temporary(SQString::Create(_ss(vm),payload));
     std::cout<<"refs "<<payload<<' '<<(_string(temporary)->_uiRef-1)<<'\n';
    } else if(op=="temp") {
     std::cout<<"temp ";value(vm->temp_reg);
    } else if(op=="release-child") {
     if(realm!=1||vm->_suspended)throw std::runtime_error("child release requires idle child");
     for(int origin:origins)if(origin==1)throw std::runtime_error("child closure still retained");
     realms[1]=nullptr;child_owner.Null();std::cout<<"child_released\n";
    } else if(op=="run"||op=="drop") {
     size_t slot;if(!(command>>slot)||slot>=closures.size()||origins[slot]!=realm)throw std::runtime_error("invalid closure owner");
     if(op=="drop") {closures[slot].Null();origins[slot]=-1;std::cout<<"dropped\n";}
     else {SQObjectPtr result;sliced(vm,closures[slot],result);}
    } else throw std::runtime_error("unknown command");
    std::string extra;if(command>>extra)throw std::runtime_error("extra command fields");
   }
  } catch(const std::exception &error) {std::cerr<<"observer_error "<<error.what()<<'\n';status=65;}
  // Closure handles are released while every owning shared state is still live.
  for(auto &closure:closures)closure.Null();
  child_owner.Null();
 }
 sq_close(independent);sq_close(root);return status;
}
