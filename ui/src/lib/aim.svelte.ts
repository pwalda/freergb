// The direction dial tells the stage to draw a passing arrow of light along the new direction.
export const aim = $state({ angle: 0, at: 0 })

export function showAim(angle: number) {
  aim.angle = angle
  aim.at = performance.now()
}
