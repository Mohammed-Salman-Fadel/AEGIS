import { useEffect, useRef } from 'react';

interface VoiceOrbProps {
  isListening: boolean;
  isSpeaking: boolean;
  isProcessing: boolean;
  analyser?: AnalyserNode;
  isDark?: boolean;
}

export function VoiceOrb({ isListening, isSpeaking, isProcessing, analyser, isDark = true }: VoiceOrbProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const requestRef = useRef<number>(0);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const draw = () => {
      const { width, height } = canvas;
      ctx.clearRect(0, 0, width, height);

      const centerX = width / 2;
      const centerY = height / 2;
      
      // Get frequency data
      let volume = 0;
      if (analyser && (isListening || isSpeaking)) {
        const dataArray = new Uint8Array(analyser.frequencyBinCount);
        analyser.getByteFrequencyData(dataArray);
        const sum = dataArray.reduce((a, b) => a + b, 0);
        volume = sum / dataArray.length;
      }

      const pulse = Math.sin(Date.now() / 400) * 5;
      const baseRadius = 40;
      const radius = baseRadius + (volume * 0.4) + pulse;

      // Draw glowing background
      const gradient = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, radius * 2.5);
      const color = isDark ? '16, 185, 129' : '16, 185, 129'; // emerald-600
      
      if (isListening || isSpeaking || isProcessing) {
        gradient.addColorStop(0, `rgba(${color}, 0.2)`);
        gradient.addColorStop(0.5, `rgba(${color}, 0.05)`);
        gradient.addColorStop(1, `rgba(${color}, 0)`);
        
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(centerX, centerY, radius * 2.5, 0, Math.PI * 2);
        ctx.fill();
      }

      // Draw Rings
      if (isListening || isSpeaking) {
        ctx.strokeStyle = `rgba(${color}, 0.3)`;
        ctx.lineWidth = 1.5;
        
        for (let i = 0; i < 3; i++) {
          const ringRadius = radius + (i * 15) + (volume * 0.2);
          const opacity = 0.3 - (i * 0.1);
          ctx.strokeStyle = `rgba(${color}, ${opacity})`;
          
          ctx.beginPath();
          ctx.arc(centerX, centerY, ringRadius, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Draw the Core
      const coreGradient = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, radius);
      coreGradient.addColorStop(0, `rgba(${color}, 0.9)`);
      coreGradient.addColorStop(1, `rgba(${color}, 0.6)`);
      
      ctx.fillStyle = coreGradient;
      ctx.shadowBlur = 15;
      ctx.shadowColor = `rgba(${color}, 0.5)`;
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
      ctx.fill();
      
      ctx.shadowBlur = 0;

      // Draw Processing Spin
      if (isProcessing) {
        ctx.strokeStyle = `rgba(${color}, 0.8)`;
        ctx.lineWidth = 3;
        ctx.beginPath();
        const startAngle = (Date.now() / 200) % (Math.PI * 2);
        ctx.arc(centerX, centerY, radius + 10, startAngle, startAngle + Math.PI / 2);
        ctx.stroke();
      }

      requestRef.current = requestAnimationFrame(draw);
    };

    draw();
    return () => cancelAnimationFrame(requestRef.current);
  }, [isListening, isSpeaking, isProcessing, analyser, isDark]);

  return (
    <div className="flex flex-col items-center justify-center p-4">
      <canvas 
        ref={canvasRef} 
        width={300} 
        height={300} 
        className="max-w-full h-auto drop-shadow-2xl"
      />
      <div className="mt-4 text-center">
        <p className={`text-sm font-medium uppercase tracking-[0.2em] ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
          {isListening ? 'Listening...' : isProcessing ? 'Processing...' : isSpeaking ? 'Speaking...' : 'Voice Mode Ready'}
        </p>
      </div>
    </div>
  );
}
