#include <CoreFoundation/CoreFoundation.h>
#include <CoreAudio/CoreAudio.h>
#include <AudioToolbox/AudioHardwareService.h>
#include <dlfcn.h>
#include <math.h>
#include <CoreGraphics/CoreGraphics.h>
#include <stdatomic.h>
#include <pthread.h>
typedef struct { float x,y; } MTPoint;
typedef struct { MTPoint position,velocity; } Readout;
typedef struct { int frame; double time; int id,state,u1,u2; Readout normalized; float size; int zero; float angle,major,minor; Readout mm; int z[2]; float u3; } Finger;
typedef int (*Frame)(void*,Finger*,int,double,int);
static void (*sink)(int,int,float,float);
static CFArrayRef devices;
static _Atomic(void*) selected;
static pthread_mutex_t lifecycle=PTHREAD_MUTEX_INITIALIZER;
static void (*stop_device)(void*);
static void (*start_device)(void*,int);
static void (*register_frame)(void*,Frame);
static void (*unregister_frame)(void*,Frame);
static CFArrayRef (*list_devices)(void);
static int (*dimensions)(void*,int*,int*);
static int frame(void *device,Finger *f,int n,double t,int index) {
    (void)t;(void)index;
    if(device!=atomic_load(&selected)) return 0;
    if(n<0 || n>32 || (n>0 && !f)) { if(sink) sink(-1,0,0,0); return 0; }
    int count=0; Finger *one=NULL;
    for(int i=0;i<n;i++) if(f[i].state==3 || f[i].state==4) { count++; one=&f[i]; }
    if(sink) {
        if(count==1) {
            if(isfinite(one->normalized.position.x) && isfinite(one->normalized.position.y))
                sink(count,one->id,one->normalized.position.x,one->normalized.position.y);
            else sink(-1,0,0,0); // Invalid samples cannot masquerade as a left-edge touch.
        } else sink(count,0,0,0);
    }
    return 0;
}
// Called without holding the Rust engine lock: stopping can wait on a callback.
int edge_restart(void) {
    if(!list_devices || !register_frame || !start_device || !dimensions || !stop_device) return -2;
    pthread_mutex_lock(&lifecycle);
    void *old=atomic_load(&selected);
    if(old && !unregister_frame) { pthread_mutex_unlock(&lifecycle); return -4; }
    old=atomic_exchange(&selected,NULL);
    if(old) { stop_device(old); unregister_frame(old,frame); }
    if(devices) { CFRelease(devices); devices=NULL; }
    devices=list_devices(); int found=0;
    if(devices) for(CFIndex i=0;i<CFArrayGetCount(devices);i++) {
        void *device=(void*)CFArrayGetValueAtIndex(devices,i);
        int rows=0,cols=0;dimensions(device,&rows,&cols);
        if(rows<10) continue;
        atomic_store(&selected,device);
        register_frame(device,frame);start_device(device,0);found=1;break;
    }
    pthread_mutex_unlock(&lifecycle);
    return found?0:-3;
}
int edge_start(void (*callback)(int,int,float,float)) {
    void *lib=dlopen("/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport",RTLD_NOW);
    if(!lib) return -1;
    list_devices=dlsym(lib,"MTDeviceCreateList");
    register_frame=dlsym(lib,"MTRegisterContactFrameCallback");
    unregister_frame=dlsym(lib,"MTUnregisterContactFrameCallback");
    start_device=dlsym(lib,"MTDeviceStart");
    dimensions=dlsym(lib,"MTDeviceGetSensorDimensions");
    stop_device=dlsym(lib,"MTDeviceStop");
    if(!list_devices || !register_frame || !start_device || !dimensions || !stop_device) return -2;
    sink=callback;
    return edge_restart();
}
int edge_option_down(void) {
    return (CGEventSourceFlagsState(kCGEventSourceStateCombinedSessionState)&kCGEventFlagMaskAlternate)!=0;
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

static int mute_state(AudioDeviceID device) {
    UInt32 muted=0,size=sizeof(muted);
    AudioObjectPropertyAddress p={kAudioDevicePropertyMute,kAudioDevicePropertyScopeOutput,kAudioObjectPropertyElementMain};
    return AudioObjectGetPropertyData(device,&p,0,NULL,&size,&muted)==0 ? (muted!=0) : -1;
}
int edge_audio_state(float *value,unsigned int *device,int *muted) {
    AudioDeviceID d=output();*device=d;*muted=d?mute_state(d):-1;
    if(!d) return -1;
    AudioObjectPropertyAddress p={kAudioHardwareServiceDeviceProperty_VirtualMainVolume,kAudioDevicePropertyScopeOutput,kAudioObjectPropertyElementMain};
    UInt32 size=sizeof(float);Boolean settable=0;
    if(!AudioObjectHasProperty(d,&p) || AudioObjectIsPropertySettable(d,&p,&settable) || !settable) return -2;
    return AudioObjectGetPropertyData(d,&p,0,NULL,&size,value);
}
int edge_adjust_volume(float delta,unsigned int expected,float *before,float *after,int *muted) {
    AudioDeviceID d=output();if(!d) return -1;
    if(d!=expected) return -4;
    if(!isfinite(delta)) return -5;
    AudioObjectPropertyAddress p={kAudioHardwareServiceDeviceProperty_VirtualMainVolume,kAudioDevicePropertyScopeOutput,kAudioObjectPropertyElementMain};
    UInt32 size=sizeof(float);Boolean settable=0;
    if(!AudioObjectHasProperty(d,&p) || AudioObjectIsPropertySettable(d,&p,&settable) || !settable) return -2;
    OSStatus code=AudioObjectGetPropertyData(d,&p,0,NULL,&size,before);if(code) return code;
    if(!isfinite(*before)) return -5;
    if(output()!=d) return -4;
    float value=fminf(1,fmaxf(0,*before+delta));
    code=AudioObjectSetPropertyData(d,&p,0,NULL,size,&value);if(code) return code;
    code=AudioObjectGetPropertyData(d,&p,0,NULL,&size,after);
    *muted=mute_state(d);
    return code;
}
