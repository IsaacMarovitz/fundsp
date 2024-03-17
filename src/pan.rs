//! Panning functionality.

use super::audionode::*;
use super::math::*;
use super::signal::*;
use super::*;
use numeric_array::*;
use std::marker::PhantomData;

/// Return equal power pan weights for pan value in -1...1.
#[inline]
fn pan_weights<T: Real>(value: T) -> (T, T) {
    let angle = (clamp11(value) + T::one()) * T::from_f64(PI * 0.25);
    (cos(angle), sin(angle))
}

/// Mono-to-stereo equal power panner. Number of inputs is `N`, either 1 or 2.
/// Setting: pan value.
/// Input 0: mono audio
/// Input 1 (optional): pan value in -1...1
/// Output 0: left output
/// Output 1: right output
#[derive(Clone)]
pub struct Panner<T: Real, N: Size<T>> {
    _marker: PhantomData<(T, N)>,
    left_weight: T,
    right_weight: T,
}

impl<T: Real, N: Size<T>> Panner<T, N> {
    pub fn new(value: T) -> Self {
        let (left_weight, right_weight) = pan_weights(value);
        Self {
            _marker: PhantomData,
            left_weight,
            right_weight,
        }
    }
    #[inline]
    pub fn set_pan(&mut self, value: T) {
        let (left_weight, right_weight) = pan_weights(value);
        self.left_weight = left_weight;
        self.right_weight = right_weight;
    }
}

impl<T: Real, N: Size<T>> AudioNode for Panner<T, N> {
    const ID: u64 = 49;
    type Sample = T;
    type Inputs = N;
    type Outputs = typenum::U2;
    type Setting = T;

    fn set(&mut self, setting: Self::Setting) {
        self.set_pan(setting);
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        if N::USIZE > 1 {
            let value = input[1];
            self.set_pan(value);
        }
        [self.left_weight * input[0], self.right_weight * input[0]].into()
    }

    fn process(
        &mut self,
        size: usize,
        input: &[&[Self::Sample]],
        output: &mut [&mut [Self::Sample]],
    ) {
        output[0][..size].clone_from_slice(&input[0][..size]);
        output[1][..size].clone_from_slice(&input[0][..size]);
        for i in 0..size {
            if N::USIZE > 1 {
                let value = input[1][i];
                let (left_weight, right_weight) = pan_weights(value);
                self.left_weight = left_weight;
                self.right_weight = right_weight;
            }
            output[0][i] *= self.left_weight;
            output[1][i] *= self.right_weight;
        }
    }

    fn route(&mut self, input: &SignalFrame, _frequency: f64) -> SignalFrame {
        let mut output = new_signal_frame(self.outputs());
        // Pretend the pan value is constant.
        output[0] = input[0].scale(self.left_weight.to_f64());
        output[1] = input[0].scale(self.right_weight.to_f64());
        output
    }
}

/// Mixing matrix with `M` input channels and `N` output channels.
#[derive(Clone)]
pub struct Mixer<M, N, T>
where
    M: Size<T>,
    N: Size<T> + Size<Frame<T, M>>,
    T: Float,
{
    matrix: Frame<Frame<T, M>, N>,
}

impl<M, N, T> Mixer<M, N, T>
where
    M: Size<T>,
    N: Size<T> + Size<Frame<T, M>>,
    T: Float,
{
    pub fn new(matrix: Frame<Frame<T, M>, N>) -> Self {
        Self { matrix }
    }
}

impl<M, N, T> AudioNode for Mixer<M, N, T>
where
    M: Size<T>,
    N: Size<T> + Size<Frame<T, M>>,
    T: Float,
{
    const ID: u64 = 84;
    type Sample = T;
    type Inputs = M;
    type Outputs = N;
    type Setting = ();

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        Frame::generate(|i| {
            let mut value = T::zero();
            for (x, y) in input.iter().zip(self.matrix[i].iter()) {
                value += (*x) * (*y);
            }
            value
        })
    }

    fn route(&mut self, input: &SignalFrame, _frequency: f64) -> SignalFrame {
        let mut output = new_signal_frame(self.outputs());
        for i in 0..self.outputs() {
            output[i] = input[0].scale(convert(self.matrix[i][0]));
            for j in 1..self.inputs() {
                output[i] = output[i].combine_linear(
                    input[j].scale(convert(self.matrix[i][j])),
                    0.0,
                    |x, y| x + y,
                    |x, y| x + y,
                );
            }
        }
        output
    }
}
