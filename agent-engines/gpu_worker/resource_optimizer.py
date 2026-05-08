from __future__ import annotations

import logging
import psutil
import os
from dataclasses import dataclass, field
from typing import Dict, Optional, List
from enum import Enum

logger = logging.getLogger("XLMate.ResourceOptimizer")


class ResourceTier(Enum):
    """Resource allocation tiers for different engine workloads."""
    LIGHT = "light"          # Minimal resources, quick responses
    STANDARD = "standard"    # Balanced performance
    HIGH = "high"           # Maximum performance for complex analysis
    UNLIMITED = "unlimited" # No constraints (testing/development)


@dataclass
class ResourceLimits:
    """Defines resource constraints for an engine instance."""
    max_threads: int = 2
    max_memory_mb: int = 512
    max_cpu_percent: float = 50.0
    max_gpu_memory_mb: int = 1024
    tier: ResourceTier = ResourceTier.STANDARD


@dataclass
class ResourceMetrics:
    """Current resource utilization metrics."""
    cpu_percent: float = 0.0
    memory_used_mb: float = 0.0
    memory_available_mb: float = 0.0
    gpu_utilization: float = 0.0
    gpu_memory_used_mb: float = 0.0
    active_workers: int = 0
    queued_tasks: int = 0


class ResourceOptimizer:
    """
    Optimizes resource allocation for AI engines based on system capacity
    and workload demands. Ensures efficient CPU/Gas utilization.
    """

    def __init__(self, reserved_cpu_percent: float = 20.0, reserved_memory_mb: int = 1024):
        """
        Initialize the resource optimizer.
        
        Args:
            reserved_cpu_percent: CPU percentage to keep reserved for system
            reserved_memory_mb: Memory (MB) to keep reserved for system
        """
        self.reserved_cpu_percent = reserved_cpu_percent
        self.reserved_memory_mb = reserved_memory_mb
        self._allocation_history: Dict[str, List[ResourceLimits]] = {}
        
    def get_system_capacity(self) -> Dict[str, float]:
        """Get total system resource capacity."""
        cpu_count = psutil.cpu_count(logical=True)
        total_memory_mb = psutil.virtual_memory().total / (1024 * 1024)
        
        return {
            "cpu_cores": cpu_count,
            "total_memory_mb": total_memory_mb,
            "available_cpu_percent": 100.0 - self.reserved_cpu_percent,
            "available_memory_mb": total_memory_mb - self.reserved_memory_mb
        }
    
    def get_current_metrics(self) -> ResourceMetrics:
        """Get current system resource utilization."""
        cpu_percent = psutil.cpu_percent(interval=0.1)
        memory = psutil.virtual_memory()
        memory_used_mb = memory.used / (1024 * 1024)
        memory_available_mb = memory.available / (1024 * 1024)
        
        # GPU metrics would require nvidia-ml-py or similar
        # Placeholder for now
        gpu_utilization = 0.0
        gpu_memory_used_mb = 0.0
        
        return ResourceMetrics(
            cpu_percent=cpu_percent,
            memory_used_mb=memory_used_mb,
            memory_available_mb=memory_available_mb,
            gpu_utilization=gpu_utilization,
            gpu_memory_used_mb=gpu_memory_used_mb
        )
    
    def calculate_optimal_allocation(
        self,
        engine_id: str,
        tier: ResourceTier = ResourceTier.STANDARD,
        current_load: float = 0.0
    ) -> ResourceLimits:
        """
        Calculate optimal resource allocation for an engine based on
        system capacity, tier requirements, and current load.
        
        Args:
            engine_id: Unique identifier for the engine
            tier: Resource tier for this engine
            current_load: Current system load (0.0 to 1.0)
            
        Returns:
            ResourceLimits with optimized allocation
        """
        capacity = self.get_system_capacity()
        metrics = self.get_current_metrics()
        
        # Calculate available resources
        available_cpu = capacity["available_cpu_percent"] - metrics.cpu_percent
        available_memory = capacity["available_memory_mb"] - metrics.memory_used_mb
        
        # Ensure we don't allocate negative resources
        available_cpu = max(0, available_cpu)
        available_memory = max(0, available_memory)
        
        # Tier-based multipliers
        tier_multipliers = {
            ResourceTier.LIGHT: 0.25,
            ResourceTier.STANDARD: 0.5,
            ResourceTier.HIGH: 0.75,
            ResourceTier.UNLIMITED: 1.0
        }
        
        multiplier = tier_multipliers[tier]
        
        # Calculate allocation based on tier and availability
        max_threads = max(1, int(capacity["cpu_cores"] * multiplier))
        max_memory = int(available_memory * multiplier)
        max_cpu = available_cpu * multiplier
        
        # Adjust based on current load
        if current_load > 0.8:
            # System is under heavy load, reduce allocation
            max_threads = max(1, max_threads // 2)
            max_memory = max_memory // 2
            max_cpu = max_cpu * 0.5
        elif current_load < 0.3:
            # System is lightly loaded, can allocate more
            max_threads = min(max_threads * 2, int(capacity["cpu_cores"]))
            max_cpu = min(max_cpu * 1.5, available_cpu)
        
        # Apply hard limits
        max_threads = min(max_threads, int(capacity["cpu_cores"]))
        max_memory = min(max_memory, int(available_memory))
        max_cpu = min(max_cpu, available_cpu)
        
        limits = ResourceLimits(
            max_threads=max_threads,
            max_memory_mb=max_memory,
            max_cpu_percent=max_cpu,
            tier=tier
        )
        
        # Track allocation history
        if engine_id not in self._allocation_history:
            self._allocation_history[engine_id] = []
        self._allocation_history[engine_id].append(limits)
        
        logger.info(
            f"Allocated resources for {engine_id} [{tier.value}]: "
            f"{max_threads} threads, {max_memory}MB RAM, {max_cpu:.1f}% CPU"
        )
        
        return limits
    
    def validate_allocation(self, limits: ResourceLimits) -> bool:
        """
        Validate that resource allocation is within safe bounds.
        
        Args:
            limits: Resource limits to validate
            
        Returns:
            True if allocation is valid, False otherwise
        """
        capacity = self.get_system_capacity()
        metrics = self.get_current_metrics()
        
        # Check CPU allocation
        if limits.max_cpu_percent > capacity["available_cpu_percent"]:
            logger.warning(f"CPU allocation {limits.max_cpu_percent}% exceeds available {capacity['available_cpu_percent']}%")
            return False
        
        # Check memory allocation
        if limits.max_memory_mb > capacity["available_memory_mb"]:
            logger.warning(f"Memory allocation {limits.max_memory_mb}MB exceeds available {capacity['available_memory_mb']}MB")
            return False
        
        # Check thread count
        if limits.max_threads > capacity["cpu_cores"]:
            logger.warning(f"Thread count {limits.max_threads} exceeds available cores {capacity['cpu_cores']}")
            return False
        
        return True
    
    def get_allocation_history(self, engine_id: Optional[str] = None) -> Dict:
        """
        Get resource allocation history.
        
        Args:
            engine_id: Optional engine ID to filter history
            
        Returns:
            Dictionary of allocation history
        """
        if engine_id:
            return {engine_id: self._allocation_history.get(engine_id, [])}
        return self._allocation_history.copy()
    
    def estimate_gas_cost(self, engine_type: str, complexity: int) -> float:
        """
        Estimate computational cost (analogous to gas) for an analysis task.
        This helps optimize resource usage and prevent expensive operations.
        
        Args:
            engine_type: Type of engine (stockfish, lc0, maia, etc.)
            complexity: Analysis depth or complexity level
            
        Returns:
            Estimated computational cost (arbitrary units)
        """
        # Base costs per engine type
        base_costs = {
            "stockfish": 10,
            "lc0": 50,  # GPU-based, more expensive
            "maia": 15,
            "custom": 20
        }
        
        base_cost = base_costs.get(engine_type.lower(), 20)
        
        # Complexity scales quadratically (deeper analysis = much more expensive)
        gas_estimate = base_cost * (complexity ** 1.5)
        
        logger.debug(f"Estimated gas cost for {engine_type} at depth {complexity}: {gas_estimate:.2f}")
        
        return gas_estimate
    
    def should_throttle(self, metrics: ResourceMetrics, threshold: float = 90.0) -> bool:
        """
        Determine if we should throttle new requests based on resource usage.
        
        Args:
            metrics: Current resource metrics
            threshold: Usage percentage threshold for throttling
            
        Returns:
            True if throttling should be applied
        """
        if metrics.cpu_percent > threshold:
            logger.warning(f"CPU usage {metrics.cpu_percent}% exceeds threshold {threshold}% - throttling recommended")
            return True
        
        if metrics.memory_used_mb > (psutil.virtual_memory().total / (1024 * 1024)) * (threshold / 100):
            logger.warning(f"Memory usage exceeds threshold {threshold}% - throttling recommended")
            return True
        
        return False
