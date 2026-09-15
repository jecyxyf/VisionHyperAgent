#version 440

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    float qt_Opacity;
    vec4 shadowColor;
    float blurRadius;
    float offsetY;
    float panelWidth;
    float panelHeight;
    float panelRadius;
    float effectWidth;
    float effectHeight;
};

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
