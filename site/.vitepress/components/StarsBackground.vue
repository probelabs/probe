<template>
  <div class="stars-background-container">
    <canvas ref="starsCanvas" class="stars-background"></canvas>
    <div v-for="planet in planets" :key="planet.type" 
         class="planet" 
         :class="planet.type"
         :style="getPlanetStyle(planet)">
    </div>
  </div>
</template>

<script>
export default {
  data() {
    return {
      planets: [
        {
          type: 'moon',
          baseScale: 0.2,
          scale: 0.2,
          position: { x: 0, y: 0 },
          opacity: 0.15,
          speed: { x: 0.02, y: 0.015 },
          direction: { x: 1, y: 1 }
        },
        {
          type: 'saturn',
          baseScale: 0.4,
          scale: 0.4,
          position: { x: 0, y: 0 },
          opacity: 0.15,
          speed: { x: 0.025, y: 0.02 },
          direction: { x: 1, y: 1 }
        },
        {
          type: 'sun',
          baseScale: 0.6,
          scale: 0.6,
          position: { x: 0, y: 0 },
          opacity: 0.15,
          speed: { x: 0.03, y: 0.018 },
          direction: { x: 1, y: 1 }
        }
      ],
      animationId: null
    }
  },
  methods: {
    getPlanetStyle(planet) {
      return {
        transform: `translate(-50%, -50%) scale(${planet.scale})`,
        top: `${planet.position.y}%`,
        left: `${planet.position.x}%`,
        opacity: planet.opacity
      }
    },
    getRandomStartPosition() {
      const side = Math.floor(Math.random() * 4) // 0: top, 1: right, 2: bottom, 3: left
      let x, y, dirX, dirY
      
      switch(side) {
        case 0: // top
          x = Math.random() * 80 + 10
          y = -10
          dirX = Math.random() * 2 - 1 // Random direction between -1 and 1
          dirY = 0.5 + Math.random() * 0.5 // Always moving down, but at an angle
          break
        case 1: // right
          x = 110
          y = Math.random() * 80 + 10
          dirX = -0.5 - Math.random() * 0.5 // Always moving left, but at an angle
          dirY = Math.random() * 2 - 1
          break
        case 2: // bottom
          x = Math.random() * 80 + 10
          y = 110
          dirX = Math.random() * 2 - 1
          dirY = -0.5 - Math.random() * 0.5 // Always moving up, but at an angle
          break
        case 3: // left
          x = -10
          y = Math.random() * 80 + 10
          dirX = 0.5 + Math.random() * 0.5 // Always moving right, but at an angle
          dirY = Math.random() * 2 - 1
          break
      }
      
      // Normalize the direction vector
      const length = Math.sqrt(dirX * dirX + dirY * dirY)
      return {
        pos: { x, y },
        dir: { x: dirX / length, y: dirY / length }
      }
    },
    updatePlanetScale(planet) {
      // Calculate distance from center (50, 50)
      const dx = planet.position.x - 50
      const dy = planet.position.y - 50
      const distance = Math.sqrt(dx * dx + dy * dy)
      
      // Max distance is from center to corner (about 70.71)
      const maxDistance = Math.sqrt(50 * 50 + 50 * 50)
      
      // Calculate scale factor based on distance
      // When at center: 1.15 (15% larger)
      // When at max distance: 0.85 (15% smaller)
      const scaleFactor = 1.15 - (0.3 * distance / maxDistance)
      
      // Apply scale factor to base scale
      planet.scale = planet.baseScale * scaleFactor
    },
    resetPlanetPosition(planet) {
      const { pos, dir } = this.getRandomStartPosition()
      planet.position = pos
      planet.direction = {
        x: dir.x / Math.sqrt(dir.x * dir.x + dir.y * dir.y),
        y: dir.y / Math.sqrt(dir.x * dir.x + dir.y * dir.y)
      }
      this.updatePlanetScale(planet)
    },
    updatePlanets() {
      this.planets.forEach(planet => {
        // Update positions
        planet.position.x += planet.speed.x * planet.direction.x
        planet.position.y += planet.speed.y * planet.direction.y

        // Update scale based on position
        this.updatePlanetScale(planet)

        // Wrap around screen edges
        if (planet.position.x < -15) {
          planet.position.x = 115
        } else if (planet.position.x > 115) {
          planet.position.x = -15
        }
        
        if (planet.position.y < -15) {
          planet.position.y = 115
        } else if (planet.position.y > 115) {
          planet.position.y = -15
        }
      })

      this.animationId = requestAnimationFrame(this.updatePlanets)
    }
  },
  mounted() {
    // Initialize random positions and directions for planets
    this.planets.forEach(planet => {
      this.resetPlanetPosition(planet)
    })

    // Start planet animation
    this.updatePlanets()

    const canvas = this.$refs.starsCanvas
    if (!canvas) {
      console.error('Stars canvas element not found')
      return
    }
    
    const ctx = canvas.getContext('2d')
    if (!ctx) {
      console.error('Could not get 2D context from canvas')
      return
    }
    
    // Set canvas dimensions
    const setCanvasSize = () => {
      canvas.width = window.innerWidth
      canvas.height = window.innerHeight
    }
    
    setCanvasSize()
    
    // Star properties
    const stars = []
    const numStars = 150 // Increased number of stars
    const trailLength = 4 // Increased trail length
    
    // Space travel simulation properties
    let currentAngle = Math.PI // Initial direction (left)
    let targetAngle = currentAngle
    let angleChangeSpeed = 0.0005 // Slower direction changes
    let baseSpeed = 1.5 // Slightly reduced speed for better visibility
    let targetSpeed = baseSpeed
    
    // Function to smoothly change travel direction
    const updateTravelDirection = () => {
      // Occasionally change target angle
      if (Math.random() < 0.0005) { // Less frequent direction changes
        targetAngle = Math.random() * Math.PI * 2
        targetSpeed = baseSpeed * (0.8 + Math.random() * 0.4) // More consistent speed
      }
      
      // Smoothly interpolate to target angle
      const angleDiff = targetAngle - currentAngle
      if (Math.abs(angleDiff) > 0.01) {
        currentAngle += angleDiff * angleChangeSpeed
      }
    }
    
    // Create stars with varied properties
    for (let i = 0; i < numStars; i++) {
      const x = Math.random() * canvas.width
      const y = Math.random() * canvas.height
      const baseSize = 2.5 + Math.random() * 3
      
      stars.push({
        x: x,
        y: y,
        z: Math.random() * 1.5,
        size: baseSize,
        baseSize: baseSize,
        color: `rgba(${230 + Math.random() * 20}, ${230 + Math.random() * 20}, ${240 + Math.random() * 15}, 1)`, // Medium bright color
        trail: Array(trailLength).fill().map(() => ({ x, y })),
        blinkSpeed: 0.01 + Math.random() * 0.015,
        blinkOffset: Math.random() * Math.PI * 2,
        shouldBlink: Math.random() < 0.4
      })
    }
    
    // Function to draw a star shape
    const drawStar = (x, y, size, color) => {
      const spikes = 4 // Reduced number of spikes for better performance
      const outerRadius = size
      const innerRadius = size * 0.4 // Increased inner radius
      
      ctx.beginPath()
      ctx.fillStyle = color
      
      for (let i = 0; i < spikes * 2; i++) {
        const radius = i % 2 === 0 ? outerRadius : innerRadius
        const angle = (Math.PI * 2 * i) / (spikes * 2) - Math.PI / 2
        
        if (i === 0) {
          ctx.moveTo(x + radius * Math.cos(angle), y + radius * Math.sin(angle))
        } else {
          ctx.lineTo(x + radius * Math.cos(angle), y + radius * Math.sin(angle))
        }
      }
      
      ctx.closePath()
      ctx.fill()
    }
    
    // Function to draw trail
    const drawTrail = (trail, color, size) => {
      for (let i = 0; i < trail.length; i++) {
        const opacity = 0.25 * ((trail.length - i) / trail.length) // Reduced trail opacity
        const trailSize = size * 0.8 * ((trail.length - i) / trail.length) // Even larger trails
        const trailColor = color.replace('1)', `${opacity})`)
        
        ctx.beginPath()
        ctx.fillStyle = trailColor
        ctx.arc(trail[i].x, trail[i].y, trailSize, 0, Math.PI * 2)
        ctx.fill()
      }
    }
    
    // Animation function
    const animate = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      
      const time = performance.now() / 1000
      updateTravelDirection()
      
      stars.forEach(star => {
        // Update trail
        star.trail.pop()
        star.trail.unshift({ x: star.x, y: star.y })
        
        // Calculate star movement based on current travel direction and star's depth
        const speed = targetSpeed * (1 + star.z)
        star.x += Math.cos(currentAngle) * speed
        star.y += Math.sin(currentAngle) * speed
        
        // Adjust star size based on speed to create depth effect
        star.size = star.baseSize * (1 + (speed - baseSpeed) * 0.1)
        
        // Calculate opacity with blinking effect
        let opacity = 1
        if (star.shouldBlink) {
          opacity = 0.85 + (Math.sin(time * star.blinkSpeed + star.blinkOffset) + 1) * 0.15 // Higher minimum opacity
        }
        
        const currentColor = star.color.replace('1)', `${opacity})`)
        
        // Draw star and trail
        drawTrail(star.trail, currentColor, star.size)
        drawStar(star.x, star.y, star.size, currentColor)
        
        // Wrap stars around screen
        if (star.x < -star.size * 2) star.x = canvas.width + star.size
        if (star.x > canvas.width + star.size * 2) star.x = -star.size
        if (star.y < -star.size * 2) star.y = canvas.height + star.size
        if (star.y > canvas.height + star.size * 2) star.y = -star.size
      })
      
      requestAnimationFrame(animate)
    }
    
    // Handle window resize
    window.addEventListener('resize', setCanvasSize)
    animate()
    
    // Store for cleanup
    this.setCanvasSize = setCanvasSize
  },
  
  beforeDestroy() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId)
    }
    if (this.setCanvasSize) {
      window.removeEventListener('resize', this.setCanvasSize)
    }
  }
}
</script>

<style>
.stars-background-container {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: -1;
  pointer-events: none;
  overflow: hidden;
}

.stars-background {
  width: 100%;
  height: 100%;
}

.planet {
  position: absolute;
  width: 300px;
  height: 300px;
  background-size: contain;
  background-repeat: no-repeat;
  background-position: center;
  filter: brightness(0.8);
  transition: transform 0.5s ease, opacity 0.3s ease;
  pointer-events: auto;
}

.planet:hover {
  filter: brightness(1);
  transform: translate(-50%, -50%) scale(1.1) !important;
  opacity: 0.9 !important;
}

.moon {
  background-image: url('/moon.png');
}

.saturn {
  background-image: url('/saturn.png');
}

.sun {
  background-image: url('/sun.png');
}
</style> 