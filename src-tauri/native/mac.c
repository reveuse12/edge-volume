#include <CoreFoundation/CoreFoundation.h>
#include <CoreAudio/CoreAudio.h>
#include <AudioToolbox/AudioHardwareService.h>
#include <dlfcn.h>
#include <math.h>
typedef struct { float x,y; } MTPoint;
typedef struct { MTPoint position,velocity; } Readout;
typedef struct { int frame; double time; int id,state,u1,u2; Readout normalized; float size; int zero; float angle,major,minor; Readout mm; int z[2]; float u3; } Finger;
typedef int (*Frame)(void*,Finger*,int,double,int);
static void (*sink)(int,int,float,float);
static CFArrayRef devices;
static void (*stop_device)(void*);
static int frame(void *device,Finger *f,int n,double t,int index) {
    (void)device;(void)t;(void)index;
    int count=0; Finger *one=NULL;
    for(int i=0;i<n;i++) if(f[i].state==3 || f[i].state==4) { count++; one=&f[i]; }
    if(sink) {
        if(count==1 && isfinite(one->normalized.position.x) && isfinite(one->normalized.position.y))
            sink(count,one->id,one->normalized.position.x,one->normalized.position.y);
        else sink(count,0,0,0);
    }
    return 0;
}
int edge_start(void (*callback)(int,int,float,float)) {
    void *lib=dlopen("/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport",RTLD_NOW);
    if(!lib) return -1;
    CFArrayRef (*list)(void)=dlsym(lib,"MTDeviceCreateList");
    void (*reg)(void*,Frame)=dlsym(lib,"MTRegisterContactFrameCallback");
    void (*start)(void*,int)=dlsym(lib,"MTDeviceStart");
    int (*dims)(void*,int*,int*)=dlsym(lib,"MTDeviceGetSensorDimensions");
    stop_device=dlsym(lib,"MTDeviceStop");
    if(!list || !reg || !start || !dims || !stop_device) return -2;
    devices=list(); if(!devices) return -3;
    sink=callback; int found=0;
    // One device per engine avoids mixing contact identities across trackpads.
    for(CFIndex i=0;i<CFArrayGetCount(devices);i++) {
        void *d=(void*)CFArrayGetValueAtIndex(devices,i); int rows=0,cols=0;
        dims(d,&rows,&cols); if(rows<10) continue;
        reg(d,frame); start(d,0); found=1; break;
    }
    return found?0:-3;
}
static AudioDeviceID output(void) {
    AudioDeviceID d=0; UInt32 size=sizeof(d);
    AudioObjectPropertyAddress p={kAudioHardwarePropertyDefaultOutputDevice,kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};
    if(AudioObjectGetPropertyData(kAudioObjectSystemObject,&p,0,NULL,&size,&d)) return 0;
    return d;
}
int edge_volume(float *value,int write) {
    AudioDeviceID d=output(); if(!d) return -1;
    AudioObjectPropertyAddress p={kAudioHardwareServiceDeviceProperty_VirtualMainVolume,kAudioDevicePropertyScopeOutput,kAudioObjectPropertyElementMain};
    UInt32 size=sizeof(float); Boolean settable=0;
    if(!AudioObjectHasProperty(d,&p)) return -2;
    if(write) {
        if(AudioObjectIsPropertySettable(d,&p,&settable) || !settable) return -2;
        return AudioObjectSetPropertyData(d,&p,0,NULL,size,value);
    }
    return AudioObjectGetPropertyData(d,&p,0,NULL,&size,value);
}
