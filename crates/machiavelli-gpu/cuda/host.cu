#include <cuda_runtime.h>
#include <cmath>
#include <cstdint>
#include <limits>
#include <vector>
#include "kernels.cu"

struct Buffer {
  double* ptr = nullptr;
  ~Buffer() { if (ptr) cudaFree(ptr); }
  bool allocate(size_t bytes) { return cudaMalloc(reinterpret_cast<void**>(&ptr), bytes) == cudaSuccess; }
};
extern "C" unsigned int machiavelli_cuda_abi_version() { return 1; }
extern "C" int machiavelli_cuda_verlet(double* q, double* p, const double* mass,
    const double* stiffness, uint64_t coordinates, uint32_t dimensions,
    double dt, uint32_t steps) {
  if (!q || !p || !mass || !stiffness || !dimensions || !coordinates ||
      coordinates % dimensions || !std::isfinite(dt) || steps > 1000000 ||
      coordinates > std::numeric_limits<size_t>::max() / sizeof(double)) return 1;
  int devices = 0;
  if (cudaGetDeviceCount(&devices) != cudaSuccess || devices < 1) return 2;
  std::vector<double> inv_mass(dimensions);
  for (uint32_t d = 0; d < dimensions; ++d) {
    if (!std::isfinite(mass[d]) || mass[d] <= 0 || !std::isfinite(stiffness[d]) || stiffness[d] < 0) return 3;
    inv_mass[d] = 1.0 / mass[d];
    if (!std::isfinite(inv_mass[d])) return 3;
  }
  size_t bytes = static_cast<size_t>(coordinates) * sizeof(double);
  size_t parameter_bytes = static_cast<size_t>(dimensions) * sizeof(double);
  Buffer device_q, device_p, device_mass, device_stiffness;
  if (!device_q.allocate(bytes) || !device_p.allocate(bytes) ||
      !device_mass.allocate(parameter_bytes) || !device_stiffness.allocate(parameter_bytes)) return 4;
  if (cudaMemcpy(device_q.ptr, q, bytes, cudaMemcpyHostToDevice) != cudaSuccess ||
      cudaMemcpy(device_p.ptr, p, bytes, cudaMemcpyHostToDevice) != cudaSuccess ||
      cudaMemcpy(device_mass.ptr, inv_mass.data(), parameter_bytes, cudaMemcpyHostToDevice) != cudaSuccess ||
      cudaMemcpy(device_stiffness.ptr, stiffness, parameter_bytes, cudaMemcpyHostToDevice) != cudaSuccess) return 5;
  uint64_t blocks = (coordinates + 255) / 256;
  if (blocks > 2147483647ULL) return 6;
  for (uint32_t step = 0; step < steps; ++step) {
    symplectic_verlet_step_kernel<<<static_cast<unsigned int>(blocks), 256>>>(
        device_q.ptr, device_p.ptr, device_mass.ptr, device_stiffness.ptr, coordinates, dimensions, dt);
    if (cudaGetLastError() != cudaSuccess) return 7;
  }
  if (cudaDeviceSynchronize() != cudaSuccess) return 8;
  if (cudaMemcpy(q, device_q.ptr, bytes, cudaMemcpyDeviceToHost) != cudaSuccess ||
      cudaMemcpy(p, device_p.ptr, bytes, cudaMemcpyDeviceToHost) != cudaSuccess) return 9;
  return 0;
}
