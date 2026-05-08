from __future__ import annotations

import logging
from typing import Dict, Any

from gpu_worker.models import PersonalityTraits

logger = logging.getLogger("XLMate.PersonalityManager")

class PersonalityManager:
    """
    Translates high-level personality traits into engine-specific configurations.
    Supports Stockfish and LC0 backends.
    """

    @staticmethod
    def traits_to_stockfish_options(traits: PersonalityTraits) -> Dict[str, Any]:
        """
        Map traits to Stockfish UCI options.
        - aggressiveness -> Contempt, Skill Level
        - positional_style -> Analysis Contempt
        - risk_tolerance -> King Safety, Mobility
        """
        # Note: These are illustrative mappings; real Stockfish options vary by version.
        options = {}
        
        # Aggressiveness: higher contempt means the engine avoids draws
        contempt = int((traits.aggressiveness - 0.5) * 100)
        options["Contempt"] = contempt
        
        # Risk Tolerance: higher risk tolerance can be simulated by lowering internal pruning thresholds
        # (This usually requires custom engine builds or hidden parameters)
        
        # Skill level can also be adjusted based on personality if desired
        
        logger.info(f"Mapped traits to Stockfish options: {options}")
        return options

    @staticmethod
    def traits_to_lc0_options(traits: PersonalityTraits) -> Dict[str, Any]:
        """
        Map traits to LC0 UCI options.
        - aggressiveness -> CPUCT
        - positional_style -> FPU Strategy
        - risk_tolerance -> Temperature
        """
        options = {}
        
        # Aggressiveness: CPUCT controls exploration vs exploitation
        cpuct = 0.8 + (traits.aggressiveness * 0.4)
        options["Cpuct"] = cpuct
        
        # Risk Tolerance: Temperature controls move selection randomness
        temp = 0.5 + (traits.risk_tolerance * 1.5)
        options["Temperature"] = temp
        
        logger.info(f"Mapped traits to LC0 options: {options}")
        return options

    @staticmethod
    def get_interaction_prompt(traits: PersonalityTraits, context: str) -> str:
        """
        Generate a system prompt for Natural Language interactions based on personality tone.
        """
        tones = {
            "neutral": "You are a professional chess assistant. Provide clear and objective analysis.",
            "aggressive": "You are a highly competitive chess grandmaster. Your analysis is sharp, bold, and focused on crushing the opponent.",
            "humorous": "You are a witty chess enthusiast. Incorporate humor and lighthearted comments into your analysis.",
            "formal": "You are a distinguished chess instructor. Provide sophisticated, detailed, and formal analysis."
        }
        
        base_prompt = tones.get(traits.tone, tones["neutral"])
        return f"{base_prompt} Context: {context}"
