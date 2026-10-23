// Reference CUDA kernels; require separate host launch/runtime integration.
// One thread per independent harmonic trajectory coordinate.
extern "C" __global__ void symplectic_verlet_step_kernel(
    double* q,double* p,const double* inv_mass,const double* stiffness,
    unsigned long long coordinates,unsigned int dimensions,double dt) {
  unsigned long long i=(unsigned long long)blockIdx.x*blockDim.x+threadIdx.x;
  if(i>=coordinates)return;
  unsigned int d=i%dimensions;
  double half=p[i]-0.5*dt*stiffness[d]*q[i];
  double position=q[i]+dt*(half*inv_mass[d]);
  q[i]=position;
  p[i]=half-0.5*dt*stiffness[d]*position;
}
// Exactly 256 threads per block; one information set per block, <=256 actions.
// Host must validate finite values and ensure offsets delimit independent sets.
extern "C" __global__ void cfr_plus_regret_matching_kernel(
    double* regrets,const double* delta,double* strategy,const unsigned int* offsets) {
  __shared__ double positive[256];
  unsigned int lane=threadIdx.x;
  unsigned int start=offsets[blockIdx.x],count=offsets[blockIdx.x+1]-start;
  double value=lane<count?fmax(0.0,regrets[start+lane]+delta[start+lane]):0.0;
  if(lane<count)regrets[start+lane]=value;
  positive[lane]=value;
  __syncthreads();
  // Normalize by maximum before summing, avoiding overflow in the reduction.
  for(unsigned int stride=128;stride>0;stride>>=1) {
    if(lane<stride)positive[lane]=fmax(positive[lane],positive[lane+stride]);
    __syncthreads();
  }
  double maximum=positive[0];
  __syncthreads();
  positive[lane]=maximum>0.0?value/maximum:0.0;
  __syncthreads();
  for(unsigned int stride=128;stride>0;stride>>=1) {
    if(lane<stride)positive[lane]+=positive[lane+stride];
    __syncthreads();
  }
  if(lane<count)strategy[start+lane]=maximum>0.0?(value/maximum)/positive[0]:1.0/count;
}
