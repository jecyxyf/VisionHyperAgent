#version 440 core

in vec2 qt_TexCoord0;
out vec4 fragColor;

uniform float qt_Opacity;
uniform vec4 shadowColor;
uniform float blurRadius;
uniform float offsetY;
uniform float panelWidth;
uniform float panelHeight;
uniform float panelRadius;
uniform float effectWidth;
uniform float effectHeight;

float roundedBoxDistance(vec2 point, vec2 halfSize, float radius)
{
    vec2 quarterSize = halfSize - vec2(radius);
    vec2 outerDistance = abs(point) - quarterSize;
    return length(max(outerDistance, vec2(0.0))) +
           min(max(outerDistance.x, outerDistance.y), 0.0) - radius;
}

void main()
{
    vec2 point = qt_TexCoord0 * vec2(effectWidth, effectHeight) -
                 vec2(effectWidth * 0.5, effectHeight * 0.5 + offsetY);
    float distance = roundedBoxDistance(point, vec2(panelWidth, panelHeight) * 0.5, panelRadius);
    float falloff = distance / blurRadius;
    float alpha = clamp(exp(-0.5 * falloff * falloff), 0.0, 1.0);
    alpha *= shadowColor.a * qt_Opacity;
    fragColor = vec4(shadowColor.rgb * alpha, alpha);
}
