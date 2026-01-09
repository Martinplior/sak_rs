#ifndef UTILS_GLSL
#define UTILS_GLSL

struct ScreenSize {
    float width;
    float height;
};

struct _UintType {
    uint MAX;
    uint MIN;
};

const _UintType Uint = _UintType(
    0xffffffff, // MAX
    0 // MIN
);

struct _IntType {
    int MAX;
    int MIN;
};

const _IntType Int = _IntType(
    0x7fffffff, // MAX
    0x80000000 // MIN
);

struct _FloatType {
    float EPSILON;
    float INFINITY;
    float NEG_INFINITY;
    float MAX;
    float MIN;
    float PI;
    float E;
};

const _FloatType Float = _FloatType(
    1.19209290e-07, // EPSILON
    1.0 / 0.0, // INFINITY
    -1.0 / 0.0, // NEG_INFINITY
    3.40282347e+38, // MAX
    -3.40282347e+38, // MIN
    3.14159265, // PI
    2.71828183 // E
);

vec2 remap(const ScreenSize screen_size, const vec2 point);

float dot_self(const vec2 v);
float dot_self(const vec3 v);
float dot_self(const vec4 v);
double dot_self(const dvec2 v);
double dot_self(const dvec3 v);
double dot_self(const dvec4 v);

float rcp(const float x);
double rcp(const double x);
vec2 rcp(const vec2 x);
vec3 rcp(const vec3 x);
vec4 rcp(const vec4 x);

int sqr(const int x);
uint sqr(const uint x);
float sqr(const float x);
double sqr(const double x);

float cross2(const vec2 a, const vec2 b);
double cross2(const dvec2 a, const dvec2 b);



// impl

vec2 remap(const ScreenSize screen_size, const vec2 screen_point) {
    const vec2 screen_vec = vec2(screen_size.width, screen_size.height);
    const float factor = 2.0;
    const float shift = -1.0;
    return screen_point / screen_vec * factor + shift;
}

#define dot_self_impl(InType, OutType) OutType dot_self(const InType v) { return dot(v, v); }

dot_self_impl(vec2, float)
dot_self_impl(vec3, float)
dot_self_impl(vec4, float)
dot_self_impl(dvec2, double)
dot_self_impl(dvec3, double)
dot_self_impl(dvec4, double)

#undef dot_self_impl

#define rcp_impl(Type) Type rcp(const Type x) { return 1.0 / x; }

rcp_impl(float)
rcp_impl(double)
rcp_impl(vec2)
rcp_impl(vec3)
rcp_impl(vec4)

#undef rcp_impl

#define sqr_impl(Type) Type sqr(const Type x) { return x * x; }

sqr_impl(int)
sqr_impl(uint)
sqr_impl(float)
sqr_impl(double)

#undef sqr_impl

#define cross2_impl(InType, OutType) \
    OutType cross2(const InType a, const InType b) { return a.x * b.y - a.y * b.x; }

cross2_impl(vec2, float)
cross2_impl(dvec2, double)

#undef cross2_impl



// impl end

#ifdef UTILS_FRAGMENT_SHADER_ONLY

float aa_step(const float edge, const float value);

float aa_step(const float edge, const float value) {
    const float rcp_sqrt2 = 0.70710678; // 1 / sqrt(2)
    const float df = length(vec2(dFdx(value), dFdy(value))) * rcp_sqrt2;
    return smoothstep(edge - df, edge + df, value);
}

#endif // UTILS_FRAGMENT_SHADER_ONLY

#endif // UTILS_GLSL
