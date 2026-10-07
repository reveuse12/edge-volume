#include <stdio.h>
#include <CoreFoundation/CoreFoundation.h>
int edge_start(void (*cb)(int,int,float,float));
int edge_volume(float*,int);
static int frames=0,contacts=0;
static void contact(int count,int id,float x,float y){(void)id;(void)x;(void)y;frames++;if(count)contacts++;}
int main(){float v=0;int audio=edge_volume(&v,0);printf("input_start=%d audio_status=%d volume=%.3f\n",edge_start(contact),audio,v);CFRunLoopRunInMode(kCFRunLoopDefaultMode,5,false);printf("frames=%d contact_frames=%d\n",frames,contacts);return 0;}
