// Passive fixed-capacity allocation identity observation. No VM operation in on_free.
#include <array>
namespace owner_trace {
struct Identity { void *pointer=nullptr; char label=0; };
struct Event { char label; SQUnsignedInteger size; };
static std::array<Identity,16> identities;
static std::array<Event,64> events;
static size_t count=0;
static bool active=false,record=false,failed=false;
static void begin(bool enabled) {
 identities={};events={};count=0;failed=false;record=enabled;active=true;
}
static bool add(void *pointer,char label) {
 for(const auto &entry:identities) if(entry.pointer==pointer || entry.label==label) {failed=true;return false;}
 for(auto &entry:identities) if(entry.label==0) {entry={pointer,label};return true;}
 failed=true;return false;
}
static void on_free(void *pointer,SQUnsignedInteger size) {
 if(!active || pointer==nullptr)return;
 for(auto &entry:identities) {
  if(entry.pointer!=pointer)continue;
  const char label=entry.label;entry.pointer=nullptr;
  if(record) {
   if(count==events.size())failed=true;
   else events[count++]={label,size};
  }
  return;
 }
}
static void *live(char label) {
 for(const auto &entry:identities)if(entry.label==label)return entry.pointer;
 failed=true;return nullptr;
}
static char label(const void *pointer) {
 if(pointer==nullptr)return '-';
 for(const auto &entry:identities)if(entry.pointer==pointer)return entry.label;
 return '?';
}
static void emit() {
 for(size_t i=0;i<count;i++)std::cout<<"free "<<events[i].label<<' '<<events[i].size<<'\n';
 count=0;
}
static bool finish() {
 emit();active=false;
 for(const auto &entry:identities)if(entry.pointer!=nullptr)failed=true;
 std::cout<<"recorder "<<(!failed)<<'\n';
 return !failed;
}
}
